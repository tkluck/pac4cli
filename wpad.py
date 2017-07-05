import platform
import configparser

import logging
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
    
    def set_logger( self, logger ):
        self.logger = logger


    @inlineCallbacks
    def get_dhcp_domains(self):
        res = []
        if 'Linux' != platform.system():
            if self.logger:
                self.logger.info("No NetworkManager available.") 
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
    def get_config_wpad_url(self, config_file):
        if config_file and os.path.isfile(config_file):
            if self.logger:
                self.logger.info("Found config file '%s'", config_file)
            config = configparser.SafeConfigParser()
            config.read(config_file)
            try:
                url = yield config.get('wpad', 'url')
                if self.logger:
                    self.logger.info("Read wpad url: %s", url)
                return url
            except configparser.NoOptionError:
                if self.logger:
                    self.logger.info("No wpad url specified")
                yield defer.succeed(None)
        else:
            yield defer.succeed(None)

    @inlineCallbacks
    def getUrls(self):
        wpad_url = yield self.get_config_wpad_url(self.config_file)
        if wpad_url is not None:
            return [ wpad_url ]
        else:
            if self.logger:
                self.logger.info("Trying to get wpad url from NetworkManager...")
            domains = yield self.get_dhcp_domains()
            return [
                "http://wpad.{}/wpad.dat".format(domain)
                for domain in domains
            ]

