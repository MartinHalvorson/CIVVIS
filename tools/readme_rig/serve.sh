#!/bin/zsh
# Start our own spectator server on a port nobody else holds, and prove the
# listener is ours before returning. Prints "PORT PID".
cd "$(dirname "$0")"
seed=${1:-1}
for port in $(seq 8930 8999); do
  if lsof -nP -iTCP:$port -sTCP:LISTEN -t >/dev/null 2>&1; then continue; fi
  nohup ./civvis play --spectate --players 6 --map grand_canals_2 --shape planet \
        --speed online --turns 500 --seed $seed --port $port --no-open \
        > "server-$port.log" 2>&1 &
  pid=$!
  for i in $(seq 1 60); do
    sleep 0.5
    owner=$(lsof -nP -iTCP:$port -sTCP:LISTEN -t 2>/dev/null | head -1)
    if [ -n "$owner" ]; then
      if [ "$owner" = "$pid" ]; then echo "$port $pid"; exit 0; fi
      echo "port $port taken by $owner, not mine ($pid)" >&2
      break
    fi
    kill -0 $pid 2>/dev/null || break
  done
  kill $pid 2>/dev/null
done
echo "no free port" >&2; exit 1
