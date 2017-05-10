import traceback

import logging
from contextlib import contextmanager

from twisted.internet import ssl
from twisted.web import proxy

from twisted.web.http import HTTPFactory
from twisted.web.client import Agent, FileBodyProducer, Headers, readBody
from twisted.internet.defer import inlineCallbacks, returnValue, Deferred

from twisted.python.compat import urllib_parse

from twisted.protocols import portforward

logger = logging.getLogger('env-pac')

import pacparser
import requests
pacparser.init()
pacparser.parse_pac_string(requests.get("http://wpad.booking.pcln.com/wpad.dat").text)

class WPADProxyRequest(proxy.ProxyRequest):
    def process(self):
            if self.method == b'CONNECT':
                uri = self.uri.decode('ascii')
                host, port = uri.split(":")
                port = int(port)

                protocol = b'http'
            else:
                parsed = urllib_parse.urlparse(self.uri)
                host = parsed[1].decode('ascii')
                protocol = parsed[0] or b'http'
                if ':' in host:
                    host, port = host.split(':')
                    port = int(port)
                else:
                    port = self.ports[protocol]
                rest = urllib_parse.urlunparse((b'', b'') + parsed[2:])
                if not rest:
                    rest = rest + b'/'

            class_ = proxy.ProxyClientFactory


            headers = self.getAllHeaders().copy()
            self.content.seek(0, 0)
            s = self.content.read()

            logger.debug("received {} request for {}".format(self.method, host))
            proxy_suggestion = pacparser.find_proxy('{}://{}'.format(protocol, host))
            logger.debug("proxy: {}".format(proxy_suggestion))

            if proxy_suggestion != 'DIRECT':
                options = proxy_suggestion.split(";")
                method, destination = options[0].split(" ")
                if method == 'PROXY':
                    proxy_host, proxy_port = destination.split(":")
                    proxy_port = int(proxy_port)
                    logger.debug("proxying to {} at port {}".format(proxy_host, proxy_port))
                    clientFactory = class_(self.method, self.uri, self.clientproto, headers,
                                           s, self)
                    self.reactor.connectTCP(proxy_host, proxy_port, clientFactory)
                    #self.reactor.connectSSL(proxy_host, proxy_port,
                    #                    clientFactory, ssl.optionsForClientTLS(host))
                    return
            if self.method != b'CONNECT':
                if b'host' not in headers:
                    headers[b'host'] = host.encode('ascii')

                clientFactory = class_(self.method, rest, self.clientproto, headers,
                                       s, self)
                if protocol == b'http':
                    self.reactor.connectTCP(host, port, clientFactory)
                elif protocol == b'https':
                    self.reactor.connectSSL(host, port,
                                        clientFactory, ssl.optionsForClientTLS(host))
            else:
                clientFactory = portforward.ProxyClientFactory()
                self.reactor.connectSSL(host, port,
                                    clientFactory, ssl.optionsForClientTLS(host))

# ***
# utility functions
# ***

def with_timeout(time, reactor, deferred):
    timeoutCall = reactor.callLater(time, deferred.cancel)
    def completed(passthrough):
        if timeoutCall.active():
            timeoutCall.cancel()
        return passthrough
    deferred.addBoth(completed)
    return deferred

def do_periodically(time, reactor, async_call):
    # Implementation note:
    # it would be great if current_d could just
    # be  a local variable, but python scoping
    # doesn't allow that (it becomes a different local
    # variable inside do_next_call, because do_next_call
    # assigns to it). That's why I'm using a workaround
    # with a dictionary element. Putting the intended
    # code in comments.
    # Python3 solves this with the 'nonlocal' keyword
    info = { 'current_d': None }             # current_d = None
    def do_next_call():                      # def do_next_call()
        if info['current_d']:                #     if current_d:
            info['current_d'].cancel()       #         current_d.cancel
        info['current_d'] = async_call()     #     current_d = async_call()
        reactor.callLater(time, do_next_call)
    reactor.callLater(time, do_next_call)

def sleep_async(time, reactor, result=None):
    d = Deferred()
    timer = reactor.callLater(time, lambda: d.callback(result))
    def on_error(err):
        timer.cancel()
        raise err
    d.addErrback(on_error)
    return d

class CleanShutdown(object):
    def __init__(self, reactor, port):
        self.port = port
        self.clean_exit_deferred = Deferred()
        self.outstanding_waits = 0
        self.stop_listening = None

        reactor.addSystemEventTrigger('before', 'shutdown', self._before_shutdown)

    def _before_shutdown(self):
        self.stop_listening = self.port.stopListening()
        self.stop_listening.addBoth(self._maybe_clean_exit)
        return self.clean_exit_deferred

    def wait_for(self, d):
        self.outstanding_waits += 1
        def on_both(result):
            self.outstanding_waits -= 1
            self._maybe_clean_exit(None)
        d.addBoth(on_both)
        return d

    def _maybe_clean_exit(self, result):
        if self.stop_listening and self.stop_listening.called and self.outstanding_waits == 0:
            self.clean_exit_deferred.callback(None)

import os
log_level_name = os.environ.get('LOG_LEVEL', 'info')
log_level = getattr(logging, log_level_name.upper(), logging.INFO)
log_handler = logging.StreamHandler()
logger.setLevel(log_level)
logger.addHandler(log_handler)
log_handler.setFormatter(logging.Formatter(fmt="%(levelname)s [%(process)d]: %(name)s: %(message)s"))
