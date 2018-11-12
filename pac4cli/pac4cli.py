import logging
logger = logging.getLogger('pac4cli')

import re

from twisted.web import proxy
from twisted.python.compat import urllib_parse
import pacparser

from . import portforward

def split_host_port(destination):
    '''
    >>> split_host_port('host')
    ('host', None)
    >>> split_host_port('host.com')
    ('host.com', None)
    >>> split_host_port('host0')
    ('host0', None)
    >>> split_host_port('host0:80')
    ('host0', 80)
    >>> split_host_port('127.0.0.1')
    ('127.0.0.1', None)
    >>> split_host_port('127.0.0.1:80')
    ('127.0.0.1', 80)
    >>> split_host_port('[0abc:1def::1234]')
    ('[0abc:1def::1234]', None)
    >>> split_host_port('[0abc:1def::1234]:443')
    ('[0abc:1def::1234]', 443)
    '''
    components = destination.rsplit(':', maxsplit=1)
    if len(components) == 1:
        return (destination, None)
    elif all(c.isdigit() for c in components[1]):
        return components[0], int(components[1])
    else:
        return (destination, None)

class WPADProxyRequest(proxy.ProxyRequest):

    force_proxy = None
    force_direct = None

    proxy_suggestion_parser = re.compile( r'(DIRECT$|PROXY) (.*)' )

    def process(self):
        method = self.method.decode('ascii')
        uri = self.uri.decode('ascii')
        if method == 'CONNECT':
            host, port = split_host_port(uri)
            port = int(port)
        else:
            parsed = urllib_parse.urlparse(self.uri)
            decoded = parsed[1].decode('ascii')
            host, port = split_host_port(decoded)
            if port is None:
                port = 80
            rest = urllib_parse.urlunparse((b'', b'') + parsed[2:])
            if not rest:
                rest = rest + b'/'

        headers = self.getAllHeaders().copy()
        self.content.seek(0, 0)
        s = self.content.read()

        proxy_suggestion = self.force_proxy or self.force_direct or pacparser.find_proxy('http://{}'.format(host))

        proxy_suggestions = proxy_suggestion.split(";")
        parsed_proxy_suggestion = self.proxy_suggestion_parser.match(proxy_suggestions[0])

        if parsed_proxy_suggestion:
            connect_method, destination = parsed_proxy_suggestion.groups()
            if connect_method == 'PROXY':
                proxy_host, proxy_port = destination.split(":")
                proxy_port = int(proxy_port)
                if method != 'CONNECT':
                    clientFactory = proxy.ProxyClientFactory(
                        self.method,
                        self.uri,
                        self.clientproto,
                        headers,
                        s,
                        self,
                    )
                    logger.info('%s %s; forwarding request to %s:%s', method, uri, proxy_host, proxy_port)
                else:
                    self.transport.unregisterProducer()
                    self.transport.pauseProducing()
                    rawConnectionProtocol = portforward.Proxy()
                    rawConnectionProtocol.transport = self.transport
                    self.transport.protocol = rawConnectionProtocol

                    clientFactory = CONNECTProtocolForwardFactory(host, port)
                    clientFactory.setServer(rawConnectionProtocol)

                    logger.info('%s %s; establishing tunnel through %s:%s', method, uri, proxy_host, proxy_port)

                self.reactor.connectTCP(proxy_host, proxy_port, clientFactory)
                return
            else:
                # can this be anything else? Let's fall back to the DIRECT
                # codepath.
                pass
        if method != 'CONNECT':
            if b'host' not in headers:
                headers[b'host'] = host.encode('ascii')

            clientFactory = proxy.ProxyClientFactory(
                self.method,
                rest,
                self.clientproto,
                headers,
                s,
                self,
            )
            logger.info('%s %s; forwarding request', method, uri)
            self.reactor.connectTCP(host, port, clientFactory)
        else:
            # hack/trick to move responsibility for this connection
            # away from a HTTP protocol class hierarchy and to a
            # port forward hierarchy
            self.transport.unregisterProducer()
            self.transport.pauseProducing()
            rawConnectionProtocol = portforward.Proxy()
            rawConnectionProtocol.transport = self.transport
            self.transport.protocol = rawConnectionProtocol

            clientFactory = portforward.ProxyClientFactory()
            clientFactory.setServer(rawConnectionProtocol)
            clientFactory.protocol = CONNECTProtocolClient
            # we don't do connectSSL, as the handshake is taken
            # care of by the client, and we only forward it
            logger.info('%s %s; establishing tunnel to %s:%s', method, uri, host, port)
            self.reactor.connectTCP(host, port,
                                clientFactory)


class CONNECTProtocolClient(portforward.ProxyClient):
    def connectionMade(self):
        self.peer.transport.write(b"HTTP/1.1 200 OK\r\n\r\n")
        portforward.ProxyClient.connectionMade(self)


class CONNECTProtocolForward(portforward.ProxyClient):
    def connectionMade(self):
        self.transport.write(
                "CONNECT {}:{} HTTP/1.1\r\nhost: {}\r\n\r\n".format(
                self.factory.host,
                self.factory.port,
                self.factory.host,
            ).encode('ascii')
        )
        portforward.ProxyClient.connectionMade(self)

class CONNECTProtocolForwardFactory(portforward.ProxyClientFactory):
    protocol = CONNECTProtocolForward
    def __init__(self, host, port):
            portforward.ProxyClientFactory.__init__(self)
            self.host = host
            self.port = port


