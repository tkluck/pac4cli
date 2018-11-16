import platform
import logging

LogHandler = logging.NullHandler


def notify_ready():
    pass


if 'Linux' == platform.system():
    import systemd.daemon
    import systemd.journal
    if hasattr(systemd.journal, 'JournalHandler'):       # official bindings
        LogHandler = systemd.journal.JournalHandler
        def notify_ready():
            systemd.daemon.notify("READY=1")
    elif hasattr(systemd.journal, 'JournaldLogHandler'):  # mosquito bindings
        LogHandler = systemd.journal.JournaldLogHandler
        def notify_ready():
            systemd.daemon.notify(systemd.daemon.Notification.READY)
    else:
        raise AssertionError("Something is wrong with the systemd module we imported")
