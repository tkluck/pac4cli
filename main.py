from twisted.internet import reactor
from twisted.web import proxy
from twisted.web.http import HTTPFactory
from twisted.web.client import Agent, FileBodyProducer, Headers, readBody
from twisted.internet import defer
from twisted.internet.defer import inlineCallbacks

from argparse import ArgumentParser

import requests
import pacparser

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
logger = logging.getLogger('env-pac')

def start_server(port, reactor):
    logger.info("Starting proxy server...")

    factory = HTTPFactory()
    factory.protocol = proxy.Proxy
    factory.protocol.requestFactory = WPADProxyRequest

    port = reactor.listenTCP(port, factory)

    logger.info("...proxy server started")

def main(args):
    try:
        pacparser.init()
        if args.force_proxy:
            WPADProxyRequest.force_proxy = args.force_proxy
        else:
            pacparser.parse_pac_string(requests.get(args.config).text)
        start_server(args.port, reactor)
    except Exception as e:
        logger.error("Problem starting the server", exc_info=True)

if __name__ == "__main__":
    import os
    log_level_name = os.environ.get('LOG_LEVEL', 'info')
    log_level = getattr(logging, log_level_name.upper(), logging.INFO)
    log_handler = logging.StreamHandler()
    logger.setLevel(log_level)
    logger.addHandler(log_handler)
    log_handler.setFormatter(logging.Formatter(fmt="%(levelname)s [%(process)d]: %(name)s: %(message)s"))
    main(args)
    reactor.run()
    logger.info("Shutdown")
