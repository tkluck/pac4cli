#!/usr/bin/env python3
from twisted.internet import reactor
from twisted.web import proxy
from twisted.web.http import HTTPFactory
from twisted.web.client import Agent, readBody
from twisted.internet import defer
from twisted.internet.defer import inlineCallbacks
from twisted.web.client import Agent

import txdbus.client
# work around txdbus assuming python 2
txdbus.client.basestring = str

from argparse import ArgumentParser

import pacparser
import signal

import systemd.daemon
import systemd.journal

parser= ArgumentParser(description="""
Run a simple HTTP proxy on localhost that uses a wpad.dat to decide
how to connect to the actual server.
""")
parser.add_argument("-c", "--config", type=str)
parser.add_argument("-p", "--port", type=int, metavar="PORT")
parser.add_argument("-F", "--force-proxy", type=str, metavar="PROXY STRING")
parser.add_argument("--loglevel", type=str, default="info", metavar="LEVEL")
parser.add_argument("--systemd", action='store_true')

args= parser.parse_args()


from pac4cli import WPADProxyRequest

import logging
logger = logging.getLogger('pac4cli')

@inlineCallbacks
def start_server(port, reactor):
    factory = HTTPFactory()
    factory.protocol = proxy.Proxy
    factory.protocol.requestFactory = WPADProxyRequest

    yield reactor.listenTCP(port, factory, interface="127.0.0.1")

    systemd.daemon.notify(systemd.daemon.Notification.READY)


@inlineCallbacks
def get_dhcp_domains():
    dbus = yield txdbus.client.connect(reactor, 'system')
    nm = yield dbus.getRemoteObject('org.freedesktop.NetworkManager',
                                   '/org/freedesktop/NetworkManager')
    active_connection_paths = yield nm.callRemote('Get',
            'org.freedesktop.NetworkManager', 'ActiveConnections')

    res = []
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
def get_possible_configuration_locations():
    if args.config:
        return [args.config]
    else:
        domains = yield get_dhcp_domains()
        return [
            "http://wpad.{}/wpad.dat".format(domain)
            for domain in domains
        ]

@inlineCallbacks
def updateWPAD(signum=None, stackframe=None):
    if args.force_proxy:
        return
    logger.info("Updating WPAD configuration...")
    wpad_urls = yield get_possible_configuration_locations()

    # use DIRECT temporarily; who knows what state the below gets pacparser
    # in
    WPADProxyRequest.force_direct = 'DIRECT'
    for wpad_url in wpad_urls:
        logger.info("Trying %s...", wpad_url)
        try:
            agent = Agent(reactor)
            # TODO: need to ensure this doesn't go through any http_proxy, such as
            # ourselves :)
            response = yield agent.request(b'GET', wpad_url.encode('ascii'))
            body = yield readBody(response)
            logger.info("...found. Parsing configuration...")
            pacparser.parse_pac_string(body.decode('ascii'))
            logger.info("Updated configuration")
            WPADProxyRequest.force_direct = None
            break
        except Exception as e:
            logger.info("...didn't work")
            pass
    else:
        logger.info("None of the tried urls seem to have worked; falling back to direct")
        WPADProxyRequest.force_direct = 'DIRECT'


@inlineCallbacks
def main(args):
    try:
        pacparser.init()
        WPADProxyRequest.force_direct = 'DIRECT' # direct, until we have a configuration
        if args.force_proxy:
            WPADProxyRequest.force_proxy = args.force_proxy
        else:
            yield updateWPAD()
        signal.signal(signal.SIGHUP, updateWPAD)
        force_proxy_message = ", sending all traffic through %s"%args.force_proxy if args.force_proxy else ""
        logger.info("Starting proxy server on port %s%s", args.port, force_proxy_message)
        yield start_server(args.port, reactor)
        logger.info("Successfully started.")
    except Exception as e:
        logger.error("Problem starting the server", exc_info=True)

if __name__ == "__main__":
    import os
    log_level_name = os.environ.get('LOG_LEVEL', args.loglevel)
    log_level = getattr(logging, log_level_name.upper(), logging.INFO)
    if args.systemd:
        log_handler = systemd.journal.JournaldLogHandler()
    else:
        log_handler = logging.StreamHandler()
    logger.setLevel(log_level)
    logger.addHandler(log_handler)
    log_handler.setFormatter(logging.Formatter(fmt="%(levelname)s [%(process)d]: %(name)s: %(message)s"))
    main(args)
    reactor.run()
    logger.info("Shutdown")
