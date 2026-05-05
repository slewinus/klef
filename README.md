# klef

Un coffre local pour tes clés API et secrets — pour arrêter de les perdre dans Dashlane, Notes, ou des `.env` éparpillés.

## Le problème

Tu as 15 clés API (Stripe, Anthropic, OpenAI, Gemini, Telnyx, etc.). Tu les notes dans Dashlane, dans des fichiers texte, dans des `.env` perdus. Quand tu démarres un projet, tu passes 10 minutes à les retrouver.

## La solution

Un CLI local qui :
- **Stocke** tes clés chiffrées (via le Keychain macOS — pas de réinvention de crypto).
- **Récupère** une clé en une commande : `klef get stripe`.
- **Injecte** les clés dans ton shell ou un projet : `eval $(klef export stripe)`.
- **Reste 100% local** — rien ne sort de ta machine.

## Workflow type

```bash
# Sauvegarder une clé (prompt interactif)
klef add stripe
> Colle ta clé : sk_live_xxxxx
> ✓ Enregistrée sous "stripe"

# Récupérer une clé
klef get stripe
sk_live_xxxxx

# Lister les clés stockées
klef list

# Injecter dans un `.env` et exécuter une commande
# .env contient : STRIPE_KEY=klef:stripe
klef run -- npm start
```

## Commandes

- `klef add <name>` — Ajouter une clé (prompt TTY ou stdin).
- `klef get <name>` — Afficher la valeur (pipeable).
- `klef show <name>` — Afficher la valeur + métadonnées.
- `klef list [--format table|json]` — Lister les clés stockées (sans les valeurs).
- `klef rm <name>` — Supprimer une clé.
- `klef edit <name>` — Changer la valeur ou les métadonnées.
- `klef rename <old> <new>` — Renommer une clé.
- `klef export <name>... [--format shell|dotenv]` — Émettre des lignes `export`.
- `klef run [--env-file FILE] -- <cmd>` — Résoudre les références `klef:<name>` dans `.env` et exécuter `<cmd>`.

## Stack

- **Langage** : Rust (édition 2024)
- **Stockage** : macOS Keychain (crate `keyring`)
- **CLI** : `clap`
- **Pas de serveur, pas de cloud, pas de compte.**

## Dev

```bash
# Setup hooks (à faire une fois)
./scripts/setup-dev.sh

# Build / test
cargo build
cargo test --all-features
cargo run -- --help
```

`cargo test --all-features` lance 39 tests : tests unitaires en `src/lib.rs` et tests E2E en `tests/cli.rs`.

Les hooks git (`fmt`, `clippy`, `test`) sont versionnés dans `.githooks/`.

## Statut

✅ v0.1 MVP — 9 commandes implémentées, 39 tests passing.

- **Spec** : [docs/design/2026-05-05-mvp-design.md](./docs/design/2026-05-05-mvp-design.md)
- **Plan d'implémentation** : [docs/plans/2026-05-05-mvp-implementation.md](./docs/plans/2026-05-05-mvp-implementation.md)
- **Plateformes** : macOS (Keychain) + Linux desktop (Secret Service via `keyring`)
- **Hors-scope v0.1** : Linux headless, Windows, synchro multi-machines
