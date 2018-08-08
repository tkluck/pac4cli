# ----------------------------------------
# A small utility script to serve a pre-fabricated http
# response to whomever connects to the port
# specified by the first argument.
#
# We used `netcat -C -l -p` before, but netcat has
# a two different version (the openbsd version and another one)
# and their names are not consistent across platforms.
# So to enable testing on more platforms, let's just
# use an equivalent script in python.
# ----------------------------------------

import socket
import sys

if __name__ == "__main__":
    port = int(sys.argv[1])
    s = socket.socket()
    s.bind(("localhost", port))
    s.listen()
    conn, addr = s.accept()

    data = sys.stdin.buffer.read()
    data = data.replace(b"\n", b"\r\n")
    conn.sendall(data)

