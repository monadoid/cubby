#!/bin/bash
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export PULSE_SERVER=unix:${XDG_RUNTIME_DIR}/pulse/native
./target/release/cubby --disable-telemetry --debug > cubby_output.log 2>&1 &
cubby_PID=$!
echo $cubby_PID > cubby.pid
# Check resource usage every 10 seconds, for 1 minute
for i in {1..6}
do
   sleep 10
   ps -p $cubby_PID -o %cpu,%mem,cmd
done
