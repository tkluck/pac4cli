import platform
import configparser

from twisted.internet.defer import inlineCallbacks
from tld import get_tld

import logging
logger = logging.getLogger('pac4cli')


if 'Linux' == platform.system():
    import txdbus.client
    # work around txdbus assuming python 2
    txdbus.client.basestring = str


_dbusclient = None
@inlineCallbacks
def get_dbus_client(reactor):
    global _dbusclient
    if _dbusclient is None or not _dbusclient.connected:
        _dbusclient = yield txdbus.client.connect(reactor, 'system')
    return _dbusclient

# TODO: move this to a more appropriate module
@inlineCallbacks
def install_network_state_changed_callback(reactor, callback):
    dbus = yield get_dbus_client(reactor)
    nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                    '/org/freedesktop/NetworkManager')
    nm.notifyOnSignal('StateChanged', callback)


class WPAD:
    def __init__(self, reactor, config_file):
        self.reactor = reactor
        self.config_file = config_file

    @inlineCallbacks
    def get_FQDNs(self):
        res = []
        if 'Linux' != platform.system():
            logger.info("No NetworkManager available.")
            return res

        dbus = yield get_dbus_client(self.reactor)
        nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                        '/org/freedesktop/NetworkManager')
        active_connection_paths = yield nm.callRemote(
            'Get', 'org.freedesktop.NetworkManager', 'ActiveConnections')

        for path in active_connection_paths:
            logger.debug("Inspecting connection %s", path)
            try:
                conn = yield dbus.getRemoteObject(
                    'org.freedesktop.NetworkManager', path)
                config_path = yield conn.callRemote(
                    'Get', 'org.freedesktop.NetworkManager.Connection.Active',
                    'Ip4Config')
                logger.debug("Its IP4 configuration is %s", config_path)
                # this is what networkmanager returns in case there is no
                # associated configuration, e.g. vpns and tunnels
                if config_path != "/":
                    config = yield dbus.getRemoteObject(
                        'org.freedesktop.NetworkManager', config_path)
                    domains = yield config.callRemote(
                        'Get', 'org.freedesktop.NetworkManager.IP4Config',
                        'Domains')
                    logger.debug("Its domains are %s", domains)
                    res.extend(domains)
                else:
                    logger.debug("Skipping /")
            except Exception as e:
                logger.warning("Problem getting domain for connection %s",
                               path, exc_info=True)

        return res

    @inlineCallbacks
    def get_dhcp_url(self):
        if 'Linux' != platform.system():
            logger.info("No NetworkManager available.")
            return None

        dbus = yield get_dbus_client(self.reactor)
        nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                        '/org/freedesktop/NetworkManager')
        active_connection_paths = yield nm.callRemote(
            'Get', 'org.freedesktop.NetworkManager', 'ActiveConnections')

        for path in active_connection_paths:
            logger.debug("Inspecting connection %s", path)
            try:
                conn = yield dbus.getRemoteObject(
                    'org.freedesktop.NetworkManager', path)
                config_path = yield conn.callRemote(
                    'Get', 'org.freedesktop.NetworkManager.Connection.Active',
                    'Dhcp4Config')
                logger.debug("Its Dhcp4 configuration is %s", config_path)

                # this is what networkmanager returns in case there is no
                # associated configuration, e.g. vpns and tunnels
                if config_path != "/":
                    config = yield dbus.getRemoteObject(
                        'org.freedesktop.NetworkManager', config_path)
                    options = yield config.callRemote(
                        'Get', 'org.freedesktop.NetworkManager.DHCP4Config',
                        'Options')
                    logger.debug("Its options are %s", options)

                    if 'wpad' in options:
                        return options['wpad']
                else:
                    logger.debug("Skipping /")
            except Exception as e:
                logger.warning("Problem getting wpad option for connection %s",
                               path, exc_info=True)

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

    def get_dns_wpad_urls(self, domains):
        dns_urls = []
        for domain in domains:
            # We need to be aware of the TLD in the passed domains, so as to 
            # avoid looking for the wpad url outside the business/company/entity
            # e.g.: for http://hostname.subdomain.example.co.uk, we want to try: 
            # http://wpad.hostname.subdomain.example.co.uk/wpad.dat
            # http://wpad.subdomain.example.co.uk/wpad.dat
            # http://wpad.example.co.uk/wpad.dat
            # and avoid trying:
            # http://wpad.co.uk/wpad.dat
            #
            # The tld package is using Mozilla's database for top level domains.
            logger.info("Found fqdn: '%s'", domain)
            tld_res = get_tld(domain, as_object=True, fix_protocol=True)
            logger.debug("tld_res.subdomain: %s", tld_res.subdomain)
            logger.debug("tld_res.fld: %s", tld_res.fld)
            domain_parts = [ p for p in tld_res.subdomain.split('.') if p != '' ]
            for i in range(len(domain_parts)):
                parts = ["wpad"] + domain_parts[i:] + [tld_res.fld]
                wpad_search_domain = '.'.join(parts)
                dns_urls.append("http://{}/wpad.dat".format(wpad_search_domain))
            dns_urls.append("http://wpad.{}/wpad.dat".format(tld_res.fld))

        return dns_urls

    @inlineCallbacks
    def getUrls(self):
        wpad_urls = []
        if self.config_file:
            try:
                wpad_url = self.get_config_wpad_url(self.config_file)
                if wpad_url is not None:
                    wpad_urls.append(wpad_url)
            except Exception as e:
                logger.warning("Problem reading configuration file %s",
                               self.config_file, exc_info=True)
        else:
            logger.debug("No configuration file specified")

        logger.info("Trying to get wpad url from NetworkManager DHCP...")
        wpad_url = yield self.get_dhcp_url()
        if wpad_url is not None:
            wpad_urls.append(wpad_url)
        else:
            logger.info("Trying to get wpad url from NetworkManager domains...")
            domains = yield self.get_FQDNs()
            dns_urls = self.get_dns_wpad_urls(domains)
            wpad_urls.extend(dns_urls)

        return wpad_urls
