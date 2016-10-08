/****************************************************************************
**
** svgcleaner could help you to clean up your SVG files
** from unnecessary data.
** Copyright (C) 2012-2016 Evgeniy Reizner
**
** This program is free software; you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation; either version 2 of the License, or
** (at your option) any later version.
**
** This program is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
**
** You should have received a copy of the GNU General Public License along
** with this program; if not, write to the Free Software Foundation, Inc.,
** 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
**
****************************************************************************/

use super::short::{EId};

use svgdom::{Document, Node, AttributeValue};

pub fn group_defs(doc: &Document) {
    // doc must contain 'svg' node, so we can safely unwrap
    let svg = doc.svg_element().unwrap();

    let defs = match doc.root().child_by_tag_id(EId::Defs) {
        Some(n) => n,
        None => {
            // create 'defs' node if it didn't exist already
            let defs = doc.create_element(EId::Defs);
            svg.prepend(defs.clone());
            defs
        }
    };

    // move all referenced elements to the main 'defs'
    {
        let mut nodes = Vec::new();
        let mut rm_nodes = Vec::new();

        for node in doc.descendants() {
            if node.is_referenced() {
                if let Some(_) = node.parent_element(EId::Mask) {
                    // All referenced elements inside mask should be removed,
                    // because it's not valid by the SVG spec. But if we ungroup such
                    // elements - they become valid, which is incorrect.
                    rm_nodes.push(node.clone());
                } else if let Some(parent) = node.parent() {
                    if parent != defs {
                        nodes.push(node.clone());
                    }
                }
            }
        }

        for n in nodes {
            resolve_attrs(&n);
            n.detach();
            defs.append(&n);
        }

        for n in rm_nodes {
            n.remove();
        }
    }

    // ungroup all existing 'defs', except main
    {
        let mut nodes = Vec::new();
        for node in doc.descendants() {
            if node.is_tag_id(EId::Defs) && node != defs {
                for child in node.children() {
                    nodes.push(child.clone());
                }
            }
        }

        for n in nodes {
            n.detach();
            defs.append(&n);
        }
    }

    // remove empty 'defs', except main
    {
        let mut nodes = Vec::new();
        for node in doc.descendants() {
            if node.is_tag_id(EId::Defs) && node != defs {
                nodes.push(node.clone());
            }
        }

        for n in nodes {
            // unneeded defs already ungrouped and must be empty
            debug_assert!(!n.has_children());
            n.remove();
        }
    }
}

// Graphical elements inside referenced elements inherits parent attributes,
// so if we want to move this elements to the 'defs' - we should resolve attributes too.
fn resolve_attrs(node: &Node) {
    match node.tag_id().unwrap() {
          EId::ClipPath
        | EId::Marker
        | EId::Mask
        | EId::Pattern
        | EId::Symbol => {
            let mut parent = node.clone();
            while let Some(p) = parent.parent() {
                let attrs = p.attributes();
                for attr in attrs.iter().filter(|a| a.is_inheritable()) {
                    for child in node.children() {
                        if child.has_attribute(attr.id) {
                            continue;
                        }

                        match attr.value {
                              AttributeValue::Link(ref link)
                            | AttributeValue::FuncLink(ref link) => {
                                // if it's fail - it's already a huge problem, so unwrap is harmless
                                child.set_link_attribute(attr.id, link.clone()).unwrap();
                            }
                            _ => child.set_attribute_object(attr.clone()),
                        }
                    }
                }

                parent = p.clone();
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svgdom::{Document, WriteToString};

    macro_rules! test {
        ($name:ident, $in_text:expr, $out_text:expr) => (
            base_test!($name, group_defs, $in_text, $out_text);
        )
    }

    // add 'defs' to 'svg' node, not to first node
    test!(create_defs_1,
b"<!--comment--><svg/>",
"<!--comment-->
<svg>
    <defs/>
</svg>
");

    test!(move_defs_1,
b"<svg>
    <altGlyphDef/>
    <clipPath/>
    <cursor/>
    <filter/>
    <linearGradient/>
    <marker/>
    <mask/>
    <pattern/>
    <radialGradient/>
    <symbol/>
    <rect/>
</svg>",
"<svg>
    <defs>
        <altGlyphDef/>
        <clipPath/>
        <cursor/>
        <filter/>
        <linearGradient/>
        <marker/>
        <mask/>
        <pattern/>
        <radialGradient/>
        <symbol/>
    </defs>
    <rect/>
</svg>
");

    // complex, recursive
    test!(move_defs_2,
b"<svg>
    <defs id='a'>
        <altGlyphDef/>
        <defs id='b'>
            <clipPath/>
        </defs>
        <defs id='c'>
            <defs id='d'>
                <cursor/>
                <filter/>
            </defs>
        </defs>
    </defs>
    <defs>
        <radialGradient/>
    </defs>
    <defs/>
</svg>",
"<svg>
    <defs id='a'>
        <altGlyphDef/>
        <clipPath/>
        <cursor/>
        <filter/>
        <radialGradient/>
    </defs>
</svg>
");

        // we should ungroup any nodes from 'defs'
    test!(move_defs_3,
b"<svg>
    <defs id='a'>
        <altGlyphDef/>
        <defs id='b'>
            <rect/>
        </defs>
    </defs>
</svg>",
"<svg>
    <defs id='a'>
        <altGlyphDef/>
        <rect/>
    </defs>
</svg>
");

    // ungroupping should only work for direct 'defs' nodes
    test!(move_defs_4,
b"<svg>
    <defs id='a'>
        <defs id='b'>
            <clipPath>
                <rect/>
            </clipPath>
        </defs>
        <line>
            <animate/>
        </line>
    </defs>
</svg>",
"<svg>
    <defs id='a'>
        <line>
            <animate/>
        </line>
        <clipPath>
            <rect/>
        </clipPath>
    </defs>
</svg>
");

    test!(move_mask_1,
b"<svg>
    <g fill='#ff0000'>
        <marker>
            <path/>
        </marker>
    </g>
</svg>",
"<svg>
    <defs>
        <marker>
            <path fill='#ff0000'/>
        </marker>
    </defs>
    <g fill='#ff0000'/>
</svg>
");

    test!(move_attrs_1,
b"<svg>
    <g fill='#ff0000'>
        <marker>
            <path/>
            <path fill='#00ff00'/>
        </marker>
    </g>
</svg>",
"<svg>
    <defs>
        <marker>
            <path fill='#ff0000'/>
            <path fill='#00ff00'/>
        </marker>
    </defs>
    <g fill='#ff0000'/>
</svg>
");

    test!(move_attrs_2,
b"<svg>
    <linearGradient id='lg1'/>
    <g fill='url(#lg1)'>
        <marker>
            <path/>
        </marker>
    </g>
</svg>",
"<svg>
    <defs>
        <linearGradient id='lg1'/>
        <marker>
            <path fill='url(#lg1)'/>
        </marker>
    </defs>
    <g fill='url(#lg1)'/>
</svg>
");

    test!(rm_inside_mask_1,
b"<svg>
    <mask>
        <linearGradient/>
        <rect/>
    </mask>
</svg>",
"<svg>
    <defs>
        <mask>
            <rect/>
        </mask>
    </defs>
</svg>
");

}
