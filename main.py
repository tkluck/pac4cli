from twisted.internet import reactor
from twisted.web import proxy
from twisted.web.http import HTTPFactory
from twisted.web.client import Agent, FileBodyProducer, Headers, readBody
from twisted.internet import defer
from twisted.internet.defer import inlineCallbacks

from proxy import WPADProxyRequest

import logging
logger = logging.getLogger('env-pac')

def start_server(reactor):
    logger.info("Starting proxy server...")

    factory = HTTPFactory()
    factory.protocol = proxy.Proxy
    factory.protocol.requestFactory = WPADProxyRequest

    port = reactor.listenTCP(3128, factory)

    logger.info("...proxy server started")

def main():
    try:
        start_server(reactor)
    except Exception as e:
        logger.error("Problem starting the server", exc_info=True)

if __name__ == "__main__":
    main()
    reactor.run()
    logger.info("Shutdown")
