#include <QWheelEvent>
#include <QShortcut>
#include <QThread>
#include <QMenu>

#include <QtDebug>

#include "wizarddialog.h"
#include "thumbwidget.h"
#include "cleanerthread.h"
#include "someutils.h"
#include "aboutdialog.h"
#include "mainwindow.h"

MainWindow::MainWindow(QWidget *parent) :
    QMainWindow(parent)
{
    setupUi(this);
    qRegisterMetaType<SVGInfo>("SVGInfo");

    // setup GUI
    actionWizard->setIcon(QIcon(":/wizard.svgz"));
    actionStart->setIcon(QIcon(":/start.svgz"));
    actionStop->setIcon(QIcon(":/stop.svgz"));
    actionThreads->setIcon(QIcon(":/cpu.svgz"));
    actionCompareView->setIcon(QIcon(":/view.svgz"));
    actionInfo->setIcon(QIcon(":/information.svgz"));
    scrollArea->installEventFilter(this);
    progressBar->hide();
    itemScroll->hide();

    // load settings
    settings = new QSettings(QSettings::NativeFormat, QSettings::UserScope,
                             "svgcleaner", "config");

    // setup threads menu
    int threadCount = settings->value("threadCount",QThread::idealThreadCount()).toInt();
    QMenu *menu = new QMenu(this);
    QActionGroup *group = new QActionGroup(actionThreads);
    for (int i = 1; i < QThread::idealThreadCount()+1; ++i) {
        QAction *action = new QAction(QString::number(i),group);
        action->setCheckable(true);
        connect(action,SIGNAL(triggered()),this,SLOT(threadsCountChanged()));
        if (i == threadCount)
            action->setChecked(true);
    }
    menu->addActions(group->actions());
    actionThreads->setMenu(menu);
    actionThreads->setToolTip(tr("Threads selected: ")+QString::number(threadCount));

    actionCompareView->setChecked(settings->value("compareView",true).toBool());

    // setup shortcuts
    QShortcut *quitShortcut = new QShortcut(QKeySequence::Quit,this);
    connect(quitShortcut,SIGNAL(activated()),qApp,SLOT(quit()));

    setWindowIcon(QIcon(":/svgcleaner.svgz"));
    on_actionWizard_triggered();

    resize(900,650);
}

MainWindow::~MainWindow()
{
}

void MainWindow::on_actionWizard_triggered()
{
    WizardDialog wizard;
    if (wizard.exec()) {
        arguments = wizard.threadArguments();
        actionStart->setEnabled(true);
    }
}

void MainWindow::on_actionStart_triggered()
{
    if (!actionStart->isEnabled())
        return;

    time = QTime::currentTime();
    time.start();

    prepareStart();
    int threadCount = settings->value("threadCount",QThread::idealThreadCount()).toInt();
    if (arguments.inputFiles.count() < threadCount)
        threadCount = arguments.inputFiles.count();

    for (int i = 0; i < threadCount; ++i) {
        QThread *thread = new QThread(this);
        CleanerThread *cleaner = new CleanerThread(arguments);
        cleanerList.append(cleaner);
        connect(cleaner,SIGNAL(cleaned(SVGInfo)),
                this,SLOT(progress(SVGInfo)),Qt::QueuedConnection);
        cleaner->moveToThread(thread);
        cleaner->startNext(arguments.inputFiles.at(position),arguments.outputFiles.at(position));
        thread->start();
        position++;
    }
}

void MainWindow::prepareStart()
{
    position = 0;
    compressMax = 0;
    compressMin = 99;
    timeMax = 0;
    timeMin = 999999999;
    inputSize = 0;
    outputSize = 0;
    averageCompress = 0;
    enableButtons(false);
    removeThumbs();
    itemScroll->hide();
    itemScroll->setMaximum(0);
    itemScroll->setValue(0);
    itemList.clear();
    progressBar->setValue(0);
    progressBar->setMaximum(arguments.inputFiles.count());
    itemLayout->addStretch(100);

    foreach (QLabel *lbl, gBoxSize->findChildren<QLabel *>(QRegExp("^lblI.*")))
        lbl->setText("0");
    foreach (QLabel *lbl, gBoxCompression->findChildren<QLabel *>(QRegExp("^lblI.*")))
        lbl->setText("0%");
    foreach (QLabel *lbl, gBoxTime->findChildren<QLabel *>(QRegExp("^lblI.*")))
        lbl->setText("00:00:00:000");
    lblITotalFiles->setText(QString::number(arguments.outputFiles.count()));
}

void MainWindow::removeThumbs()
{
    QLayoutItem *item;
    while ((item = itemLayout->takeAt(0)) != 0)
        item->widget()->deleteLater();
}

void MainWindow::enableButtons(bool value)
{
    actionStart->setEnabled(value);
    actionStop->setEnabled(!value);
    actionWizard->setEnabled(value);
    actionThreads->setEnabled(value);
    actionInfo->setEnabled(value);
    progressBar->setVisible(!value);
}

void MainWindow::progress(SVGInfo info)
{
    itemList.append(info);
    if (info.crashed)
        lblICrashed->setText(QString::number(lblICrashed->text().toInt()+1));
    else {
        averageCompress += info.compress;
        inputSize += info.sizes[SVGInfo::INPUT];
        outputSize += info.sizes[SVGInfo::OUTPUT];
        if (info.compress > compressMax && info.compress < 100) compressMax = info.compress;
        if (info.compress < compressMin && info.compress > 0) compressMin = info.compress;
        if (info.time > timeMax) timeMax = info.time;
        if (info.time < timeMin) timeMin = info.time;
    }

    int available = scrollArea->height()/itemLayout->itemAt(0)->geometry().height();
    if (available >= itemLayout->count() || itemLayout->isEmpty()) {
        itemLayout->insertWidget(itemLayout->count()-1,
                                 new ThumbWidget(info,actionCompareView->isChecked()));
    } else {
        itemScroll->show();
        itemScroll->setMaximum(itemScroll->maximum()+1);
        if (itemScroll->value() == itemScroll->maximum()-1)
            itemScroll->setValue(itemScroll->value()+1);
    }
    createStatistics();

    progressBar->setValue(progressBar->value()+1);

    CleanerThread *cleaner = qobject_cast<CleanerThread *>(sender());

    if (position < arguments.inputFiles.count()) {
        cleaner->startNext(arguments.inputFiles.at(position),arguments.outputFiles.at(position));
        position++;
    } else
        cleaningFinished();
}

void MainWindow::on_actionStop_triggered()
{
    if (!actionStop->isEnabled())
        return;

    foreach (QThread *th, findChildren<QThread *>()) {
        th->terminate();
        th->deleteLater();
    }
    enableButtons(true);
}

void MainWindow::cleaningFinished()
{
    foreach (QThread *th, findChildren<QThread *>()) {
        th->terminate();
        th->deleteLater();
    }
    enableButtons(true);
}

void MainWindow::on_itemScroll_valueChanged(int value)
{
//    QTimer::singleShot(100, this, SLOT(scrollTo(int)));
    foreach (ThumbWidget *item, findChildren<ThumbWidget *>())
        item->refill(itemList.at(value++),actionCompareView->isChecked());
}

void MainWindow::scrollTo(int value)
{
//    foreach (ThumbWidget *item, findChildren<ThumbWidget *>())
//        item->refill(itemList.at(value++),actionCompareView->isChecked());
}


void MainWindow::createStatistics()
{
    SomeUtils utils;

    // files
    lblICleaned->setText(QString::number(progressBar->value()-lblICrashed->text().toInt()+1));
    lblITotalSizeBefore->setText(utils.prepareSize(inputSize));
    lblITotalSizeAfter->setText(utils.prepareSize(outputSize));

    // cleaned
    lblIAverComp->setText(QString::number(
                              100-(averageCompress/lblICleaned->text().toInt()),'f',2)+"%");
    lblIMaxCompress->setText(QString::number(100-compressMin,'f',2)+"%");
    lblIMinCompress->setText(QString::number(100-compressMax,'f',2)+"%");

    // time
    int fullTime = time.elapsed();
    lblIFullTime->setText(utils.prepareTime(fullTime));
    lblIMaxTime->setText(utils.prepareTime(timeMax));
    lblIAverageTime->setText(utils.prepareTime(fullTime/lblICleaned->text().toInt()));
    lblIMinTime->setText(utils.prepareTime(timeMin));
}

void MainWindow::threadsCountChanged()
{
    QAction *action = qobject_cast<QAction *>(sender());
    settings->setValue("threadCount",action->text());
    actionThreads->setToolTip(tr("Threads selected: ")+action->text());
}

void MainWindow::on_actionCompareView_triggered()
{
    int i = itemScroll->value();
    foreach (ThumbWidget *item, findChildren<ThumbWidget *>())
        item->refill(itemList.at(i++),actionCompareView->isChecked());
    settings->setValue("compareView",actionCompareView->isChecked());
}

void MainWindow::on_actionInfo_triggered()
{
    AboutDialog dialog;
    dialog.exec();
}

bool MainWindow::eventFilter(QObject *obj, QEvent *event)
{
    if (obj == scrollArea && event->type() == QEvent::Wheel) {
        QWheelEvent *wheelEvent = static_cast<QWheelEvent*>(event);
        if (wheelEvent->delta() > 0)
            itemScroll->setValue(itemScroll->value()-1);
        else
            itemScroll->setValue(itemScroll->value()+1);
        return true;
    }
    return false;
}

void MainWindow::resizeEvent(QResizeEvent *)
{
    if (itemList.isEmpty())
        return;

    if (scrollAreaWidgetContents->height() > scrollArea->height()) {
        // 2 - because the last widget is spacer
        QWidget *w = itemLayout->takeAt(itemLayout->count()-2)->widget();
        itemLayout->removeWidget(w);
        w->deleteLater();
    } else {
        int available = scrollArea->height()/itemLayout->itemAt(0)->geometry().height();
        if (available >= itemLayout->count() && itemList.count() > available) {
            // this is a slow method, but it works
            QList<ThumbWidget *> list = findChildren<ThumbWidget *>();
            itemLayout->insertWidget(itemLayout->count()-1,
                                     new ThumbWidget(itemList.at(list.count()),
                                                     actionCompareView->isChecked()));
        }
    }
    itemScroll->setMaximum(itemList.count()-itemLayout->count()+1);
}

void MainWindow::closeEvent(QCloseEvent *)
{
    foreach (QThread *th, findChildren<QThread *>()) {
        th->quit();
        th->wait();
        delete th;
    }
    exit(0);
}
