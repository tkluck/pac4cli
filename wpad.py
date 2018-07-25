import platform
import configparser

import logging
logger = logging.getLogger('pac4cli')
import os

from twisted.internet import reactor
from twisted.internet.defer import inlineCallbacks
from twisted.internet import defer

if 'Linux' == platform.system():
    import txdbus.client
    # work around txdbus assuming python 2
    txdbus.client.basestring = str

class WPAD:
    def __init__(self, reactor, config_file):
        self.reactor = reactor
        self.config_file = config_file

    @inlineCallbacks
    def get_dhcp_domains(self):
        res = []
        if 'Linux' != platform.system():
            logger.info("No NetworkManager available.")
            return res

        dbus = yield txdbus.client.connect(self.reactor, 'system')
        nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                       '/org/freedesktop/NetworkManager')
        active_connection_paths = yield nm.callRemote('Get',
                'org.freedesktop.NetworkManager', 'ActiveConnections')

        for path in active_connection_paths:
            conn = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                              path)
            config_path = yield conn.callRemote('Get',
                        'org.freedesktop.NetworkManager.Connection.Active', 'Ip4Config')
            config = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                config_path)
            domains = yield config.callRemote('Get',
                    'org.freedesktop.NetworkManager.IP4Config', 'Domains')
            res.extend(domains)

        return res

    @inlineCallbacks
    def get_wpad_url(self):
        if 'Linux' != platform.system():
            logger.info("No NetworkManager available.")
            return None

        dbus = yield txdbus.client.connect(self.reactor, 'system')
        nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                       '/org/freedesktop/NetworkManager')
        active_connection_paths = yield nm.callRemote('Get',
                'org.freedesktop.NetworkManager', 'ActiveConnections')

        for path in active_connection_paths:
            conn = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                              path)
            config_path = yield conn.callRemote('Get',
                        'org.freedesktop.NetworkManager.Connection.Active', 'Dhcp4Config')

            config = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                config_path)
            options = yield config.callRemote('Get',
                    'org.freedesktop.NetworkManager.DHCP4Config', 'Options')

            if 'wpad' in options:
                return options['wpad']

        return None

    @inlineCallbacks
    def get_config_wpad_url(self, config_file):
        if config_file and os.path.isfile(config_file):
            logger.info("Found config file '%s'", config_file)
            config = configparser.SafeConfigParser()
            config.read(config_file)
            try:
                url = yield config.get('wpad', 'url')
                logger.info("Read wpad url: %s", url)
                return url
            except configparser.NoOptionError:
                logger.info("No wpad url specified")
                yield defer.succeed(None)
        else:
            yield defer.succeed(None)

    @inlineCallbacks
    def getUrls(self):
        wpad_url = yield self.get_config_wpad_url(self.config_file)
        if wpad_url is not None:
            return [ wpad_url ]
        else:
            logger.info("Trying to get wpad url from NetworkManager DHCP...")
            wpad_url = yield self.get_wpad_url()
            if wpad_url is not None:
                return [ wpad_url ]
            else:
                logger.info("Trying to get wpad url from NetworkManager domains...")
                domains = yield self.get_dhcp_domains()
                return [
                    "http://wpad.{}/wpad.dat".format(domain)
                    for domain in domains
                ]
