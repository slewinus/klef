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
# Sauvegarder une clé
klef add stripe
> Colle ta clé : sk_live_xxxxx
> ✓ Enregistrée sous "stripe"

# Récupérer
klef get stripe
sk_live_xxxxx

# Injecter dans le shell courant
eval $(klef export stripe anthropic openai)
```

## Stack

- **Langage** : Rust (édition 2024)
- **Stockage** : macOS Keychain (crate `keyring`)
- **CLI** : `clap` (à valider en brainstorming)
- **Pas de serveur, pas de cloud, pas de compte.**

## Dev

```bash
# Setup hooks (à faire une fois)
./scripts/setup-dev.sh

# Build / test
cargo build
cargo test
cargo run -- --help
```

Les hooks git (`fmt`, `clippy`, `test`) sont versionnés dans `.githooks/`.

## Statut

🚧 En conception — voir [SPEC.md](./SPEC.md) et [ROADMAP.md](./ROADMAP.md).
