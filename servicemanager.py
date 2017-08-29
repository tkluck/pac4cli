import platform
import logging

if 'Linux' == platform.system():
    import systemd.daemon
    import systemd.journal

def notify_ready():
    if 'Linux' == platform.system():
        systemd.daemon.notify(systemd.daemon.Notification.READY)

def getLogHandler():
    if 'Linux' == platform.system():
        return systemd.journal.JournaldLogHandler()
    else:
        return logging.NullHandler()
