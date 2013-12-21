/****************************************************************************
**
** SVG Cleaner is batch, tunable, crossplatform SVG cleaning program.
** Copyright (C) 2013 Evgeniy Reizner
** Copyright (C) 2012 Andrey Bayrak, Evgeniy Reizner
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

#ifndef BASECLEANER_H
#define BASECLEANER_H

#include "tools.h"

class BaseCleaner
{
public:
    BaseCleaner(XMLDocument *doc);
    virtual ~BaseCleaner() {}
    XMLDocument* document() const;
    SvgElement svgElement() const;
    SvgElement defsElement() const;
    void updateXLinks(const StringHash &hash);

private:
    XMLDocument *m_doc;
    SvgElement m_svgElem;
    SvgElement m_defsElem;
};

#endif // BASECLEANER_H