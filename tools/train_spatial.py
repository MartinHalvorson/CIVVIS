#!/usr/bin/env python3
"""Train a spatial value net on `civvis selfplay` output.

Consumes the tensor dump written by ``civvis selfplay --out <dir>``:
a residual CNN over the map planes is fused with the global scalar vector
and trained to predict the game outcome from a fog-honest position.

    civvis selfplay --games 200 --out selfplay/run1
    python tools/train_spatial.py selfplay/run1 --epochs 40

Writes ``<dir>/spatial_value.pt`` only when an untouched game-grouped test
split beats the constant baseline on both BCE and accuracy.  The report is
always written, so a rejected experiment remains auditable. Requires PyTorch;
uses CUDA when available (the point of the exercise on this rig).

The map wraps east-west, so horizontal convolutions use circular padding
and vertical ones zero-pad — a flat 'same' padding would teach the net that
the west edge borders empty space, which it does not.
"""
import argparse
import json
import os

import numpy as np


def load(dir_path):
    meta = json.load(open(os.path.join(dir_path, "meta.json")))
    planes = np.fromfile(os.path.join(dir_path, "planes.f32"), dtype="<f4")
    globals_ = np.fromfile(os.path.join(dir_path, "globals.f32"), dtype="<f4")
    labels = np.fromfile(os.path.join(dir_path, "labels.f32"), dtype="<f4")
    planes = planes.reshape(meta["planes_shape"])
    globals_ = globals_.reshape(meta["globals_shape"])
    labels = labels.reshape(meta["labels_shape"])
    if not len(planes):
        raise SystemExit(f"{dir_path}: no samples; run civvis selfplay first")
    return meta, planes, globals_, labels


def build(meta, channels=64, blocks=4):
    import torch
    from torch import nn

    n_planes = meta["planes_shape"][1]
    n_globals = meta["globals_shape"][1]

    class WrapConv(nn.Module):
        """3x3 convolution that wraps horizontally like the game map."""

        def __init__(self, cin, cout):
            super().__init__()
            self.conv = nn.Conv2d(cin, cout, 3, padding=0)

        def forward(self, x):
            x = torch.nn.functional.pad(x, (1, 1, 0, 0), mode="circular")
            x = torch.nn.functional.pad(x, (0, 0, 1, 1), mode="constant", value=0.0)
            return self.conv(x)

    class Block(nn.Module):
        def __init__(self, c):
            super().__init__()
            self.a, self.b = WrapConv(c, c), WrapConv(c, c)
            self.na, self.nb = nn.BatchNorm2d(c), nn.BatchNorm2d(c)

        def forward(self, x):
            y = torch.relu(self.na(self.a(x)))
            y = self.nb(self.b(y))
            return torch.relu(x + y)

    class Net(nn.Module):
        def __init__(self):
            super().__init__()
            self.stem = WrapConv(n_planes, channels)
            self.stem_norm = nn.BatchNorm2d(channels)
            self.blocks = nn.Sequential(*[Block(channels) for _ in range(blocks)])
            self.head = nn.Sequential(
                nn.Linear(channels * 2 + n_globals, 128), nn.ReLU(), nn.Linear(128, 1)
            )

        def forward(self, planes, globals_):
            x = torch.relu(self.stem_norm(self.stem(planes)))
            x = self.blocks(x)
            pooled = torch.cat(
                [x.mean(dim=(2, 3)), x.amax(dim=(2, 3)), globals_], dim=1
            )
            return self.head(pooled)

    return Net()


def split_by_game(labels, rng):
    """Return disjoint train/validation/test indices grouped by source game."""
    games = labels[:, 2].astype(int)
    unique = np.unique(games)
    if len(unique) < 10:
        raise SystemExit("need at least 10 distinct games for train/validation/test splits")
    rng.shuffle(unique)
    test_games = set(unique[: max(1, len(unique) // 5)].tolist())
    validation_games = set(
        unique[len(test_games) : len(test_games) + max(1, len(unique) // 5)].tolist()
    )
    train_games = set(unique[len(test_games) + len(validation_games) :].tolist())
    indices = np.arange(len(labels))
    train = indices[np.array([game in train_games for game in games])]
    validation = indices[np.array([game in validation_games for game in games])]
    test = indices[np.array([game in test_games for game in games])]
    if not len(train) or not len(validation) or not len(test):
        raise SystemExit("every game split must contain samples")
    return train, validation, test, len(train_games), len(validation_games), len(test_games)


def metrics(logits, labels, base_rate):
    """Return BCE and threshold accuracy for model or constant predictions."""
    eps = 1e-7
    probabilities = np.clip(1 / (1 + np.exp(-np.clip(logits, -60, 60))), eps, 1 - eps)
    bce = float(-(labels * np.log(probabilities) + (1 - labels) * np.log(1 - probabilities)).mean())
    accuracy = float(((probabilities >= 0.5) == labels).mean())
    constant = min(max(base_rate, eps), 1 - eps)
    baseline_bce = float(-(labels * np.log(constant) + (1 - labels) * np.log(1 - constant)).mean())
    baseline_accuracy = float(max(labels.mean(), 1 - labels.mean()))
    return bce, accuracy, baseline_bce, baseline_accuracy


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--epochs", type=int, default=40)
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--channels", type=int, default=64)
    ap.add_argument("--blocks", type=int, default=4)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()

    try:
        import torch
        from torch import nn
    except ImportError:
        raise SystemExit("PyTorch required: pip install torch")

    meta, planes, globals_, labels = load(args.dir)
    torch.manual_seed(args.seed)
    rng = np.random.default_rng(args.seed)
    # Split BY GAME. Sibling counterfactual lanes and late snapshots share
    # a source game's outcome, so splitting individual rows would leak the
    # answer. Validation selects an epoch; test remains untouched until then.
    train_idx, val_idx, test_idx, train_games, val_games, test_games = split_by_game(
        labels, rng
    )
    print(
        f"{train_games}/{val_games}/{test_games} train/validation/test games -> "
        f"{len(train_idx)}/{len(val_idx)}/{len(test_idx)} samples"
    )

    dev = "cuda" if torch.cuda.is_available() else "cpu"
    net = build(meta, args.channels, args.blocks).to(dev)
    opt = torch.optim.Adam(net.parameters(), lr=1e-3)
    loss_fn = nn.BCEWithLogitsLoss()

    def tensors(idx):
        return (
            torch.tensor(planes[idx]).to(dev),
            torch.tensor(globals_[idx]).to(dev),
            torch.tensor(labels[idx, :1]).to(dev),
        )

    best, best_state, patience = float("inf"), None, 0
    for epoch in range(args.epochs):
        net.train()
        shuffled = rng.permutation(train_idx)
        for i in range(0, len(shuffled), args.batch):
            p, g, y = tensors(shuffled[i : i + args.batch])
            opt.zero_grad()
            loss = loss_fn(net(p, g), y)
            loss.backward()
            opt.step()
        net.eval()
        with torch.no_grad():
            vp, vg, vy = tensors(val_idx)
            logits = net(vp, vg).squeeze(1).cpu().numpy()
            vloss, acc, _, _ = metrics(logits, labels[val_idx, 0], 0.5)
        print(f"epoch {epoch:3d}  val BCE {vloss:.4f}  acc {acc:.3f}")
        if vloss < best - 1e-5:
            best, patience = vloss, 0
            best_state = {k: v.detach().cpu().clone() for k, v in net.state_dict().items()}
            best_acc = acc
        else:
            patience += 1
            if patience >= 8:
                break

    # Evaluate the chosen checkpoint exactly once on untouched games. A
    # spatial artifact is useful only if it clearly beats the train-rate
    # constant on both proper scoring and classification, not merely during
    # model selection on validation.
    base_rate = float(labels[train_idx, 0].mean())
    net.load_state_dict(best_state)
    net.eval()
    with torch.no_grad():
        tp, tg, _ = tensors(test_idx)
        test_logits = net(tp, tg).squeeze(1).cpu().numpy()
    test_bce, test_acc, baseline_bce, baseline_acc = metrics(
        test_logits, labels[test_idx, 0], base_rate
    )
    beat = test_bce < baseline_bce - 1e-4 and test_acc > baseline_acc + 1e-4
    print(
        f"baseline (constant p={base_rate:.3f}): BCE {baseline_bce:.4f} "
        f"acc {baseline_acc:.3f}; test: BCE {test_bce:.4f} acc {test_acc:.3f} "
        f"-> model {'BEATS' if beat else 'DOES NOT BEAT'} baseline"
    )

    out = os.path.join(args.dir, "spatial_value.pt")
    report = {"validation_bce": best, "validation_acc": best_acc,
              "test_bce": test_bce, "test_acc": test_acc, "device": dev,
              "baseline_bce": baseline_bce, "baseline_acc": baseline_acc,
              "beats_baseline": bool(beat), "samples": int(len(planes)),
              "train": int(len(train_idx)), "validation": int(len(val_idx)),
              "test": int(len(test_idx)), "train_games": train_games,
              "validation_games": val_games, "test_games": test_games}
    json.dump(report, open(os.path.join(args.dir, "train_report.json"), "w"), indent=2)
    if beat:
        torch.save({"state_dict": best_state, "meta": meta,
                    "channels": args.channels, "blocks": args.blocks}, out)
        print(f"wrote {out}: test BCE {test_bce:.4f}, acc {test_acc:.3f} on {dev}")
    else:
        print(f"rejected: no spatial artifact written; report saved to {args.dir}")


if __name__ == "__main__":
    main()
