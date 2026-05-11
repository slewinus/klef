# klef

> **klef stocke tes clés API dans le Keychain de l'OS et les résout au runtime dans tes `.env` (`STRIPE_KEY=klef:stripe`). Pas de mot de passe maître, pas de cloud, pas de plaintext sur disque.**

[![Crates.io](https://img.shields.io/crates/v/klef.svg)](https://crates.io/crates/klef)
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

## Pourquoi pas un autre outil ?

| | klef | 1Password CLI | doppler / infisical | direnv + .env |
|---|---|---|---|---|
| Local-first | ✅ | ❌ (compte 1P) | ❌ (cloud) | ✅ |
| Stockage Keychain natif | ✅ | via `op` | ❌ | ❌ |
| Références dans `.env` | ✅ `klef:` | ✅ `op://` | ✅ `{{var}}` | ❌ littéral |
| Pas de mot de passe maître | ✅ (Touch ID) | ❌ | ❌ | ✅ |
| Gratuit | ✅ | $3/mois | freemium | ✅ |
| Multi-machine sync | ❌ (v0.4) | ✅ | ✅ | ❌ |

klef cible le cas mono-utilisateur, mono-machine, local-first, gratuit. Les concurrents sont excellents — c'est juste une niche différente. (Comparaison vérifiée le 2026-05-06.)

## Démo

```bash
# Tu as un .env qui traîne avec des secrets en clair :
$ cat .env
STRIPE_API_KEY=sk_live_xyz
ANTHROPIC_API_KEY=sk-ant-zzz
PORT=3000

# Une commande pour tout importer dans le Keychain et réécrire le .env en références :
$ klef import .env --rewrite
ENV VAR             KLEF NAME             VALUE
STRIPE_API_KEY      stripe-api-key        sk_l*** (16 chars)
ANTHROPIC_API_KEY   anthropic-api-key     sk-a*** (12 chars)
PORT                port                  *** (4 chars)
Import 3 key(s)? [y/N] y
✓ STRIPE_API_KEY → klef:stripe-api-key
✓ ANTHROPIC_API_KEY → klef:anthropic-api-key
✓ PORT → klef:port
Imported 3 key(s).
Rewrote .env (3 reference(s) replaced).

$ cat .env
STRIPE_API_KEY=klef:stripe-api-key
ANTHROPIC_API_KEY=klef:anthropic-api-key
PORT=klef:port

# Maintenant lance ton app comme avant — klef résout les références à la volée :
$ klef run -- node app.js
Server on port 3000, Stripe wired ✓
```

[![asciicast](https://asciinema.org/a/5z9zCmNWd1igb3MH.svg)](https://asciinema.org/a/5z9zCmNWd1igb3MH)

_Cast source : [`docs/klef-demo.cast`](./docs/klef-demo.cast) — re-uploadable si asciinema.org tombe._

## Install

### Cargo (recommandé)

```bash
cargo install klef
```

### Homebrew (macOS / Linux desktop)

```bash
brew tap slewinus/tap
brew install klef
```

### Binaires pré-compilés

Disponibles sur la [page Releases](https://github.com/slewinus/klef/releases) — macOS Intel + Apple Silicon, Linux x86_64 + ARM. Décompresser et déplacer dans le `$PATH`.

### Auto-complétion shell

```bash
# zsh
klef completions zsh > ~/.zfunc/_klef

# bash
klef completions bash > /usr/local/etc/bash_completion.d/klef

# fish
klef completions fish > ~/.config/fish/completions/klef.fish
```

> La complétion statique des sous-commandes et des flags fonctionne dès aujourd'hui. La complétion dynamique des noms de clés (ex. `klef get <TAB>`) est suivie dans [#28](https://github.com/slewinus/klef/issues/28) et pas encore implémentée.

## Commandes

| Commande | Rôle |
|---|---|
| `klef add <name>` | Ajouter une clé (prompt TTY ou stdin). Avec `--value-from-file <FILE>` pour les secrets multi-lignes (PEM, JSON). |
| `klef get <name>` | Afficher la valeur (pipeable). |
| `klef show <name>` | Valeur + métadonnées. |
| `klef list [--format table\|json] [-v\|--verbose] [--filter PATTERN]` | Lister les clés (jamais les valeurs). `--verbose` ajoute la date d'ajout, `--filter` cherche en sous-chaîne. |
| `klef rm <name>` (alias `remove`) | Supprimer une clé. |
| `klef edit <name>` | Changer la valeur ou les métadonnées. `--value-from-file` pour les secrets multi-lignes. `--note-edit` ouvre `$VISUAL`/`$EDITOR` pour éditer la note. |
| `klef set-note <name> <text>` | Raccourci pour `edit --note`. |
| `klef rename <old> <new>` | Renommer une clé. |
| `klef export <name>... [--format shell\|dotenv]` | Émettre des lignes `export`. |
| `klef import <file.env> [--prefix P] [--dry-run] [--rewrite] [--yes]` | Bulk-import depuis un `.env` existant. `--rewrite` remplace les valeurs littérales par des références `klef:` dans le fichier source. |
| `klef run [--env-file FILE] -- <cmd>` | Résoudre `klef:<name>` dans `.env` et exec `<cmd>`. |
| `klef status [--format text\|json]` | Diagnostic : version, backend, index path, nombre de clés, désync. Exit 1 si désync. |
| `klef completions <shell>` | Générer le script d'auto-complétion. |

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

# Build / test (cargo workspace : klef-core + klef-cli + klef-gui)
cargo build --workspace
cargo test --workspace --all-features
cargo run -p klef -- --help
```

### GUI (klef-gui)

Le crate Tauri a un frontend Svelte qui doit être bundlé avant un `cargo build/run -p klef-gui` (parce que `tauri::generate_context!` valide `frontendDist` au compile time) :

```bash
cd crates/klef-gui
npm ci                # une fois
npm run build         # à chaque modif du frontend (ou utiliser `npm run dev` en parallèle de cargo run)
cd ../..
cargo run -p klef-gui # menu bar mode : icône en haut à droite, clic pour ouvrir (pas d'icône Dock — LSUIElement=true)
```

Les hooks git (`fmt`, `clippy`, `tests`, line-cap < 300 lignes/fichier) sont versionnés dans `.githooks/`. CI sur macOS + Ubuntu via GitHub Actions (`.github/workflows/ci.yml`).

## Documentation

- **Quickstart** : [examples/quickstart/](./examples/quickstart/) — `.env` + script consommateur, smoke test bout-en-bout.
- **macOS users** : si tu vois des prompts de mot de passe fréquents, lance `klef keychain configure` une fois — voir [`docs/macos-keychain.md`](./docs/macos-keychain.md).
- **Changelog** : [CHANGELOG.md](./CHANGELOG.md)

## Pour les agents IA

klef expose une documentation pensée pour les assistants IA :

- **[`llms.txt`](./llms.txt)** : index de navigation (convention [llmstxt.org](https://llmstxt.org/))
- **[`llms-full.txt`](./llms-full.txt)** : doc concaténée pour ingestion en un prompt
- **[`docs/llm-usage.md`](./docs/llm-usage.md)** : patterns concrets — décision table, exit codes, JSON outputs
- **[`docs/mcp.md`](./docs/mcp.md)** : MCP server (`klef mcp`) — let Claude/Cursor use your keys without ever seeing the plaintext value.

Les agents Claude Code, Cursor, ChatGPT peuvent ingérer ces fichiers et savoir comment piloter klef sans hallucination.

## Statut

✅ **v0.3.0** taggé (2026-05-06) — release majeure avec onboarding bulk + backup chiffré + organisation par tags. 14 commandes, distribution complète (cargo, Homebrew, binaires pré-compilés).

Nouveautés v0.3 :
- `klef discover` scanne récursivement et propose un import bulk de tous les `.env` trouvés (avec `--skip-defaults` pour exclure le config non-secret)
- `klef backup vault.age` + `klef restore` — dump chiffré complet via [age](https://github.com/FiloSottile/age), restore atomique 3-phases
- **Tags** pour organiser : `klef add --tag billing --tag prod`, `klef list --tag ai`, `klef tags` pour le sommaire
- `klef edit --note-edit` ouvre `$EDITOR` pour des notes multi-lignes
- Complétion dynamique des noms de clés sur **bash + fish** (zsh shipped en v0.2)
- Doc IA-friendly (`llms.txt`) suivant la convention [llmstxt.org](https://llmstxt.org/)

- **Plateformes supportées** : macOS (Keychain natif) + Linux desktop (Secret Service via gnome-keyring / KWallet) + Linux headless / CI / Docker via `--backend age:./vault.age` (closes [#12](https://github.com/slewinus/klef/issues/12)).
- **Hors-scope v0.3** : Windows, synchro multi-machines, GUI.
- **Roadmap** : voir [issues by milestone](https://github.com/slewinus/klef/milestones). v0.4+ : [#24](https://github.com/slewinus/klef/issues/24) MCP server, [#18](https://github.com/slewinus/klef/issues/18) GUI.

## Licence

[MIT](./LICENSE) — © 2026 Oscar R.
