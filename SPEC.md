# Spécifications — klef

## Principes

1. **Local d'abord** — tout vit sur la machine de l'utilisateur, jamais sur un serveur.
2. **Pas de mot de passe maître** — on délègue au Keychain macOS, qui gère déjà déverrouillage/Touch ID.
3. **CLI minimaliste** — quelques commandes, faciles à mémoriser et à scripter.
4. **Lisible par d'autres outils** — export en variables d'env, en `.env`, en JSON.

## Commandes MVP (v0.1)

### `klef add <name>`
Ajoute une clé. Lit la valeur depuis stdin (pour pouvoir coller sans laisser la clé dans l'historique shell).

```
$ klef add stripe
Colle la clé (Ctrl+D pour valider) :
sk_live_xxxxx
✓ "stripe" enregistrée
```

Options :
- `--note "clé prod du compte besle"` — note libre attachée
- `--force` — écrase si existe déjà

### `klef get <name>`
Affiche la valeur d'une clé. Par défaut sur stdout (donc pipeable).

```
$ klef get stripe
sk_live_xxxxx

$ klef get stripe | pbcopy   # copie dans le presse-papier
```

### `klef list`
Liste les clés stockées (noms + notes uniquement, jamais les valeurs).

```
$ klef list
NAME       NOTE                    ADDED
stripe     prod compte besle       2026-05-01
anthropic  perso                   2026-05-03
gemini     -                       2026-05-04
```

### `klef rm <name>`
Supprime une clé (avec confirmation).

### `klef export <name>...`
Imprime des `export VAR=value` pour eval dans le shell.

```
$ eval $(klef export stripe anthropic)
$ echo $STRIPE_KEY
sk_live_xxxxx
$ echo $ANTHROPIC_KEY
sk-ant-xxxxx
```

Convention de nommage : `<NAME>_KEY` en majuscules. Surcouchable via `--var STRIPE_API_KEY`.

## Stockage

- Backend : **macOS Keychain** (service = `klef`, account = nom de la clé).
- Métadonnées (notes, date d'ajout) : fichier JSON `~/.config/klef/index.json` — non sensible, juste l'index.
- Aucune valeur de clé n'est jamais écrite sur disque en clair.

## Hors-scope MVP

- Synchro multi-machines (plus tard, peut-être via iCloud Keychain natif).
- TUI / GUI (peut-être v0.3).
- Linux / Windows (v0.2 — Secret Service / Credential Manager).
- Rotation automatique, expiration, audit.

## Décisions à trancher (brainstorming)

- **Format pour `add`** : stdin (sécurisé, scriptable) vs prompt interactif masqué (`rpassword`). Probablement les deux selon le contexte (TTY ou pas).
- **Nom de la variable d'env pour `export`** : convention stricte (`<NAME>_KEY`) ou config par clé ?
- **Crate CLI** : `clap` (standard, dérivable) vs `argh` (plus léger).
- **Index local** : JSON simple ou SQLite ? JSON suffit largement pour l'échelle attendue (<100 clés).
- **Confirmation `rm`** : prompt par défaut + flag `--yes` pour bypass.
