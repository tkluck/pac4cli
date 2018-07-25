import platform
import configparser

import logging
logger = logging.getLogger('pac4cli')
import os

from twisted.internet import reactor
from twisted.internet.defer import inlineCallbacks

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
            try:
                conn = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                  path)
                config_path = yield conn.callRemote('Get',
                            'org.freedesktop.NetworkManager.Connection.Active', 'Ip4Config')
                # this is what networkmanager returns in case there is no associated
                # configuration, e.g. vpns and tunnels
                if config_path != "/":
                    config = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                        config_path)
                    domains = yield config.callRemote('Get',
                            'org.freedesktop.NetworkManager.IP4Config', 'Domains')
                    res.extend(domains)
            except Exception as e:
                logger.warning("Problem getting domain for connection %s", path, exc_info=True)

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
            try:
                conn = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                  path)
                config_path = yield conn.callRemote('Get',
                            'org.freedesktop.NetworkManager.Connection.Active', 'Dhcp4Config')

                # this is what networkmanager returns in case there is no associated
                # configuration, e.g. vpns and tunnels
                if config_path != "/":
                    config = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                                        config_path)
                    options = yield config.callRemote('Get',
                            'org.freedesktop.NetworkManager.DHCP4Config', 'Options')

                    if 'wpad' in options:
                        return options['wpad']
            except Exception as e:
                logger.warning("Problem getting wpad option for connection %s", path, exc_info=True)

        return None

    def get_config_wpad_url(self, config_file):
        logger.info("Trying to read config file '%s'", config_file)
        config = configparser.SafeConfigParser()
        config.read(config_file)
        try:
            url = config.get('wpad', 'url')
            logger.info("Read wpad url: %s", url)
            return url
        except configparser.NoOptionError:
            logger.info("No wpad url specified")
            return None

    @inlineCallbacks
    def getUrls(self):
        if self.config_file:
            try:
                wpad_url = self.get_config_wpad_url(self.config_file)
                if wpad_url is not None:
                    return [ wpad_url ]
            except Exception as e:
                logger.warning("Problem reading configuration file %s", self.config_file, exc_info=True)

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
