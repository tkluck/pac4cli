import logging
logger = logging.getLogger('pac4cli')

from argparse import ArgumentParser
from os import path
import os
import tempfile
import signal
import subprocess

from twisted.internet import reactor
from twisted.web.client import Agent, readBody
from twisted.internet.defer import inlineCallbacks

from .wpad import WPAD, install_network_state_changed_callback
from . import servicemanager


parser= ArgumentParser(description="""
Run a simple HTTP proxy on localhost that uses a wpad.dat to decide
how to connect to the actual server.
""")
parser.add_argument("-c", "--config", type=str)
parser.add_argument("-b", "--bind", type=str, metavar="ADDRESS", default="127.0.0.1")
parser.add_argument("-p", "--port", type=int, metavar="PORT")
parser.add_argument("-F", "--force-proxy", type=str, metavar="PROXY STRING")
parser.add_argument("--loglevel", type=str, default="info", metavar="LEVEL")
parser.add_argument("--systemd", action='store_true')
parser.add_argument("--runtimedir", type=str, default=tempfile.mkdtemp())

args = parser.parse_args()

server_process = None

def start_server(interface, port, reactor):
    write_pac_file(None)
    tinyproxy_conf_path = path.join(args.runtimedir, "tinyproxy.conf")
    with open(tinyproxy_conf_path, "w") as config:
        config.write("""
            Listen {interface}
            Port {port}
            MaxClients 1
            StartServers 1
            PacUpstream "{pac_filename}"
        """.format(
            interface=interface,
            port=port,
            pac_filename=path.join(args.runtimedir, "wpad.dat")))
    global server_process
    server_process = subprocess.Popen(["/home/tkluck/src/pac4cli/tinyproxy/src/tinyproxy",
        "-d", "-c", tinyproxy_conf_path])
    # TODO: make tinyproxy send this signal
    servicemanager.notify_ready()

def write_pac_file(script):
    if script is None:
        script = b"""
            function FindProxyForURL(url, host) {
                return "DIRECT";
            }
        """
    pac_file_path = path.join(args.runtimedir, "wpad.dat")
    with open(pac_file_path, "wb") as pac_file:
        pac_file.write(script)
        os.sync()
    if server_process is not None:
        server_process.send_signal(signal.SIGHUP)

@inlineCallbacks
def get_possible_configuration_locations():
    try:
        wpad = WPAD( reactor, args.config )
        urls = yield wpad.getUrls()
        return urls
    except Exception as e:
        logger.warning("Issue getting wpad configuration", exc_info=True)
        return []

@inlineCallbacks
def updateWPAD(signum=None, stackframe=None):
    if args.force_proxy:
        return
    logger.info("Updating WPAD configuration...")
    wpad_urls = yield get_possible_configuration_locations()

    write_pac_file(None)
    for wpad_url in wpad_urls:
        logger.info("Trying %s...", wpad_url)
        try:
            agent = Agent(reactor)
            # TODO: need to ensure this doesn't go through any http_proxy, such as
            # ourselves :)
            response = yield agent.request(b'GET', wpad_url.encode('ascii'))
            body = yield readBody(response)
            logger.info("...found. Parsing configuration...")
            write_pac_file(body)
            logger.info("Updated configuration")
            break
        except Exception as e:
            logger.info("...didn't work", exc_info=True)
            pass
    else:
        logger.info("None of the tried urls seem to have worked; falling back to direct")


@inlineCallbacks
def main(args):
    try:
        if args.force_proxy:
            # TODO: pass to tinyproxy configuration
            pass

        try:
            yield install_network_state_changed_callback(reactor, updateWPAD)
        except Exception as e:
            # It _may_ actually be preferable to just die if we can't register
            # this handler. However, the test scripts use a mocked version of
            # dbus (python-dbusmock) which doesn't support mocking signals. So
            # I'll just let this pass as a warning for that case.
            logger.warning("Issue registering for network state change notifications", exc_info=True)

        force_proxy_message = ", sending all traffic through %s"%args.force_proxy if args.force_proxy else ""
        logger.info("Starting proxy server on %s:%s%s", args.bind, args.port, force_proxy_message)
        yield start_server(args.bind, args.port, reactor)
        logger.info("Successfully started; getting first configuration.")
        yield updateWPAD()
        logger.info("Have first configuration.")
    except Exception as e:
        logger.error("Problem starting the server", exc_info=True)

if __name__ == "__main__":
    import os
    log_level_name = os.environ.get('LOG_LEVEL', args.loglevel)
    log_level = getattr(logging, log_level_name.upper(), logging.INFO)
    if args.systemd:
        log_handler = servicemanager.LogHandler()
    else:
        log_handler = logging.StreamHandler()
    logger.setLevel(log_level)
    logger.addHandler(log_handler)
    log_handler.setFormatter(logging.Formatter(fmt="%(levelname)s [%(process)d]: %(name)s: %(message)s"))
    main(args)
    reactor.run()
    logger.info("Shutdown...")
    if server_process is not None:
        logger.info("Terminating server process...")
        server_process.terminate()
        server_process.wait()
    logger.info("Shutdown complete")
