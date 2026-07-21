"""Elo tournament harness: evaluate AI strategies against each other.

Every finished game is scored as pairwise results between the seated
entrants ordered by final placement (winner first, then score), with the
standard Elo update scaled by 1/(n-1) so multiplayer games count like a
set of head-to-head matches.

Custom strategies: pass any factory returning an object with
take_turn(game, pid); builtin names "basic"/"random" can map to None.

    from civvis.elo import run_tournament, leaderboard
    pool = run_tournament({"basic": None, "mybot": lambda seed: MyBot(seed)},
                          games=40)
    print(leaderboard(pool))
"""
import random

from .ai import make_ai
from .game import Game

BUILTIN_AIS = ("basic", "random")


def expected(ra, rb):
    return 1.0 / (1.0 + 10 ** ((rb - ra) / 400.0))


class EloPool:
    def __init__(self, names, base=1000.0):
        self.ratings = {n: float(base) for n in names}
        self.games = {n: 0 for n in names}
        self.wins = {n: 0 for n in names}

    def record(self, placements, k=24.0):
        """placements: entrant names ordered best -> worst for one game."""
        n = len(placements)
        if n < 2:
            return
        delta = {name: 0.0 for name in set(placements)}
        for i, a in enumerate(placements):
            for j in range(i + 1, n):
                b = placements[j]
                if a == b:
                    continue
                ea = expected(self.ratings[a], self.ratings[b])
                gain = k / (n - 1) * (1.0 - ea)
                delta[a] += gain
                delta[b] -= gain
        for name, d in delta.items():
            self.ratings[name] += d
        for idx, name in enumerate(placements):
            self.games[name] += 1
            if idx == 0:
                self.wins[name] += 1


def _run(game, ais):
    while game.winner is None:
        pid = game.current
        ais[pid].take_turn(game, pid)
        if game.winner is None and game.current == pid:
            game.apply(pid, {"type": "end_turn"})
    return game


def run_tournament(entrants, games=20, players_per_game=4, width=24, height=16,
                   max_turns=150, num_city_states=2, seed=0, k=24.0,
                   verbose=True):
    """entrants: dict name -> ai_factory(seed) (None = builtin by name)."""
    names = list(entrants)
    if not names:
        raise ValueError("no entrants")
    rng = random.Random(seed)
    pool = EloPool(names)
    for gi in range(games):
        gseed = seed * 100000 + gi
        seats = [names[rng.randrange(len(names))] for _ in range(players_per_game)]
        if len(set(seats)) == 1 and len(names) > 1:
            others = [n for n in names if n != seats[0]]
            seats[rng.randrange(players_per_game)] = others[rng.randrange(len(others))]
        game = Game(num_players=players_per_game, width=width, height=height,
                    seed=gseed, max_turns=max_turns,
                    num_city_states=num_city_states)
        ais = {}
        for p in game.players:
            if p.id < players_per_game:
                fac = entrants[seats[p.id]]
                ais[p.id] = fac(gseed + p.id) if fac else \
                    make_ai(seats[p.id], seed=gseed + p.id)
            else:
                ais[p.id] = make_ai("basic", seed=gseed + p.id)
        _run(game, ais)
        ranked = sorted(range(players_per_game),
                        key=lambda pid: (pid != game.winner, -game.score(pid), pid))
        pool.record([seats[pid] for pid in ranked], k=k)
        if verbose:
            wname = seats[game.winner] if game.winner < players_per_game \
                else game.players[game.winner].civ
            print(f"game {gi:3d}  winner={wname:<10} "
                  f"({game.players[game.winner].civ}, {game.victory_type}, "
                  f"t{game.turn})  seats={seats}")
    return pool


def leaderboard(pool):
    lines = ["Elo leaderboard:"]
    for name, r in sorted(pool.ratings.items(), key=lambda x: (-x[1], x[0])):
        g, w = pool.games[name], pool.wins[name]
        lines.append(f"  {name:<14} {r:7.1f}   games={g:<4} wins={w:<4} "
                     f"winrate={w / max(1, g):.0%}")
    return "\n".join(lines)
