from twisted.internet import reactor
from twisted.web import proxy
from twisted.web.http import HTTPFactory
from twisted.web.client import Agent, FileBodyProducer, Headers, readBody
from twisted.internet import defer
from twisted.internet.defer import inlineCallbacks
from twisted.web.client import Agent

from argparse import ArgumentParser

import requests
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

args= parser.parse_args()


from proxy import WPADProxyRequest

import logging
logger = logging.getLogger('pac4cli')

@inlineCallbacks
def start_server(port, reactor):
    factory = HTTPFactory()
    factory.protocol = proxy.Proxy
    factory.protocol.requestFactory = WPADProxyRequest

    yield reactor.listenTCP(port, factory)

    systemd.daemon.notify(systemd.daemon.Notification.READY)

@inlineCallbacks
def updateWPAD(signum=None, stackframe=None):
    try:
        logger.info("Updating WPAD configuration...")
        agent = Agent(reactor)
        # TODO: need to ensure this doesn't go through any http_proxy, such as
        # ourselves :)
        response = yield agent.request(b'GET', b'http://nu.nl/') # args.config
        logger.info("Updated configuration.")
        WPADProxyRequest.force_direct = None
    except Exception as e:
        logger.error("Problem updating configuration; falling back to direct", exc_info=True)
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
    log_level_name = os.environ.get('LOG_LEVEL', 'info')
    log_level = getattr(logging, log_level_name.upper(), logging.INFO)
    log_handler = systemd.journal.JournaldLogHandler()
    logger.setLevel(log_level)
    logger.addHandler(log_handler)
    log_handler.setFormatter(logging.Formatter(fmt="%(levelname)s [%(process)d]: %(name)s: %(message)s"))
    main(args)
    reactor.run()
    logger.info("Shutdown")
