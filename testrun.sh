#!/bin/bash
PORT1=$1
PORT2=$(( $1 + 1 ))


env/bin/python main.py -p $PORT1  -F DIRECT &
PID1=$!
env/bin/python main.py -p $PORT2 -F "PROXY localhost:$PORT1" &
PID2=$!
sleep 0.3

(
    http_proxy=localhost:$PORT1 curl http://booking.com &&
    https_proxy=localhost:$PORT1 curl https://booking.com &&
    http_proxy=localhost:$PORT2 curl http://booking.com &&
    https_proxy=localhost:$PORT2 curl https://booking.com
)
RET=$?

kill $PID1
kill $PID2

wait

exit $RET
