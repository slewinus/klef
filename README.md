# klef

[![CI](https://github.com/slewinus/klef/actions/workflows/ci.yml/badge.svg)](https://github.com/slewinus/klef/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-lightgrey)](#statut)

Un coffre local pour tes clés API et secrets — pour arrêter de les perdre dans Dashlane, Notes, ou des `.env` éparpillés.

## Le problème

Tu as 15 clés API (Stripe, Anthropic, OpenAI, Gemini, Telnyx, etc.). Tu les notes dans Dashlane, dans des fichiers texte, dans des `.env` perdus. Quand tu démarres un projet, tu passes 10 minutes à les retrouver — et le pire, tu copies-colles la valeur dans le `.env` du projet, donc elle traîne en clair sur ton disque.

## La solution

Un CLI local qui :
- **Stocke** tes clés dans le Keychain de l'OS — chiffrement géré par Apple/GNOME, pas par nous.
- **Récupère** une clé en une commande : `klef get stripe`.
- **Injecte** les valeurs dans tes projets via des **références** dans le `.env` plutôt que des valeurs en clair :
  ```
  STRIPE_KEY=klef:stripe          # référence — résolue au runtime
  ANTHROPIC_KEY=klef:anthropic    # idem
  ```
  Puis `klef run -- npm start` résout tout ça et exécute ta commande avec les bonnes vars d'env.
- **Reste 100% local** — pas de serveur, pas de cloud, pas de télémétrie.

## Démo

```bash
# 1. Sauvegarder ta clé Stripe (prompt masqué façon sudo)
$ klef add stripe
Paste value for 'stripe': ********
✓ 'stripe' saved

# 2. Dans le .env du projet, remplacer la vraie clé par une référence
$ cat .env
STRIPE_KEY=klef:stripe
PORT=3000

# 3. Lancer ton app — klef résout, exec ta commande
$ klef run -- node app.js
Server on port 3000, Stripe wired ✓
```

> 🎬 _Asciinema cast en cours d'enregistrement — voir [examples/quickstart/](./examples/quickstart/) pour le scénario complet en attendant._

## Install

### Depuis les sources (Rust ≥ 1.85)

```bash
cargo install --git https://github.com/slewinus/klef
```

### Homebrew (macOS)

`brew install slewinus/tap/klef` — _en cours de packaging, voir [#10](https://github.com/slewinus/klef/issues/10)._

### Binaires pré-compilés

À venir, voir [#11](https://github.com/slewinus/klef/issues/11).

### Auto-complétion shell

```bash
# zsh
klef completions zsh > ~/.zfunc/_klef

# bash
klef completions bash > /usr/local/etc/bash_completion.d/klef

# fish
klef completions fish > ~/.config/fish/completions/klef.fish
```

## Commandes

| Commande | Rôle |
|---|---|
| `klef add <name>` | Ajouter une clé (prompt TTY ou stdin). |
| `klef get <name>` | Afficher la valeur (pipeable). |
| `klef show <name>` | Valeur + métadonnées. |
| `klef list [--format table\|json]` | Lister les clés (jamais les valeurs). |
| `klef rm <name>` | Supprimer une clé. |
| `klef edit <name>` | Changer la valeur ou les métadonnées. |
| `klef rename <old> <new>` | Renommer une clé. |
| `klef export <name>... [--format shell\|dotenv]` | Émettre des lignes `export`. |
| `klef run [--env-file FILE] -- <cmd>` | Résoudre `klef:<name>` dans `.env` et exec `<cmd>`. |
| `klef completions <shell>` | Générer le script d'auto-complétion shell. |

`klef --help` ou `klef <cmd> --help` pour les détails de chaque option.

## Stack

- **Langage** : Rust (édition 2024)
- **Stockage** : Keychain natif via [`keyring`](https://crates.io/crates/keyring) — Apple Security framework sur macOS, Secret Service sur Linux.
- **CLI** : [`clap`](https://crates.io/crates/clap) (derive)
- **Pas de serveur, pas de cloud, pas de compte, pas de télémétrie.**

## Dev

```bash
# Setup hooks (à faire une fois après le clone)
./scripts/setup-dev.sh

# Build / test
cargo build
cargo test --all-features      # 43 tests : unit + E2E
cargo run -- --help
```

Les hooks git (`fmt`, `clippy`, `tests`, line-cap < 300 lignes/fichier) sont versionnés dans `.githooks/`. CI sur macOS + Ubuntu via GitHub Actions (`.github/workflows/ci.yml`).

## Documentation

- **Design spec** (10 décisions tranchées + architecture) : [docs/design/2026-05-05-mvp-design.md](./docs/design/2026-05-05-mvp-design.md)
- **Plan d'implémentation** (17 tasks, TDD) : [docs/plans/2026-05-05-mvp-implementation.md](./docs/plans/2026-05-05-mvp-implementation.md)
- **Quickstart** : [examples/quickstart/](./examples/quickstart/) — `.env` + script consommateur, smoke test bout-en-bout.
- **Changelog** : [CHANGELOG.md](./CHANGELOG.md)

## Statut

✅ **v0.1 MVP** — 10 commandes, 43 tests passing, killer feature `klef run` validée bout-en-bout.

- **Plateformes supportées** : macOS (Keychain natif) + Linux desktop (Secret Service via gnome-keyring / KWallet).
- **Hors-scope v0.1** : Linux headless / WSL sans desktop, Windows, synchro multi-machines, GUI.
- **Roadmap** : voir [issues by milestone](https://github.com/slewinus/klef/milestones) — v0.1 (release polish), v0.2 (`klef import`, Homebrew, binaires pré-compilés), v0.3+ (backend chiffré `age`, sync iCloud, GUI).

## Licence

[MIT](./LICENSE) — © 2026 Oscar R.
