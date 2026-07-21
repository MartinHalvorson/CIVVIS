from civvis.elo import EloPool, expected, leaderboard, run_tournament


def test_expected_symmetry():
    assert abs(expected(1000, 1000) - 0.5) < 1e-9
    assert expected(1200, 1000) > 0.7


def test_record_zero_sum_and_direction():
    pool = EloPool(["a", "b", "c"])
    pool.record(["a", "b", "c"])
    assert pool.ratings["a"] > 1000 > pool.ratings["c"]
    assert abs(sum(pool.ratings.values()) - 3000) < 1e-6
    assert pool.wins["a"] == 1 and pool.games["b"] == 1


def test_basic_beats_random():
    pool = run_tournament({"basic": None, "random": None}, games=8,
                          players_per_game=2, width=16, height=12,
                          max_turns=90, num_city_states=0, seed=1,
                          verbose=False)
    assert pool.ratings["basic"] > pool.ratings["random"]
    assert "Elo leaderboard" in leaderboard(pool)


def test_tournament_deterministic():
    kw = dict(games=3, players_per_game=2, width=16, height=12, max_turns=25,
              num_city_states=0, seed=7, verbose=False)
    a = run_tournament({"basic": None, "random": None}, **kw)
    b = run_tournament({"basic": None, "random": None}, **kw)
    assert a.ratings == b.ratings
