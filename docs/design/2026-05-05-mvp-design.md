# klef — MVP Design (v0.1)

**Date :** 2026-05-05
**Statut :** En revue
**Périmètre :** Première version utilisable du CLI klef.

Ce document gèle les décisions de design issues du brainstorming. Toute déviation pendant l'implémentation doit être renvoyée ici sous forme de PR de mise à jour.

---

## 1. Vision en une phrase

Un CLI local-first qui stocke tes clés API dans le keychain de l'OS et les injecte dans tes projets via des références (`klef:stripe`) plutôt que des valeurs en clair dans tes `.env`.

## 2. Principes de design

1. **Local d'abord** — aucune valeur ne quitte la machine.
2. **Pas de mot de passe maître** — on délègue l'auth au Keychain de l'OS (Touch ID inclus).
3. **CLI minimaliste** — peu de commandes, chacune avec un rôle distinct, mémorisables.
4. **Référence > valeur** — `klef run` est la voie royale : pas de secret en clair dans `.env`.
5. **YAGNI** — pas de feature spéculative en v0.1.

## 3. Périmètre v0.1

### 3.1 Plateformes supportées

| Plateforme | Statut MVP | Backend |
|---|---|---|
| macOS | ✅ | Keychain (via `keyring`) |
| Linux desktop (GNOME/KDE) | ✅ | Secret Service (via `keyring`) |
| Linux headless / WSL sans desktop | ❌ | v0.3 (backend fichier chiffré) |
| Windows | ❌ | v0.2+ (Credential Manager) |

### 3.2 Modèle de données

- **Plat** : un nom = une valeur. Pas d'environnements / profils intégrés.
- L'utilisateur préfixe à la main si besoin (`stripe-prod`, `stripe-test`).

### 3.3 Commandes (9 au total)

| Commande | Rôle |
|---|---|
| `klef add <name> [--as VAR] [--note ...] [--force]` | Ajoute une clé. Prompt masqué si TTY, lit stdin sinon. |
| `klef get <name>` | Imprime la valeur sur stdout. Newline si TTY, sans si pipe. |
| `klef show <name>` | Affiche la valeur formatée pour lecture humaine. |
| `klef list [--format table\|json]` | Liste noms + notes + dates. Jamais les valeurs. |
| `klef rm <name> [--yes]` | Supprime. Prompt par défaut, `--yes` pour bypass. |
| `klef edit <name> [--note ...] [--as VAR]` | Modifie valeur (re-prompt) et/ou métadonnées. |
| `klef rename <old> <new>` | Renomme une clé (Keychain + index). |
| `klef export <name>... [--format shell\|dotenv]` | Imprime `export VAR=value` pour `eval`, ou format `.env`. |
| `klef run [--env-file FILE] -- <cmd>` | Lit `.env`, résout les `klef:<name>`, exec la commande. |

### 3.4 Convention de nommage des variables d'env

- **Défaut** : `<NAME>_API_KEY` en majuscules. Couvre Stripe (`STRIPE_API_KEY`), OpenAI (`OPENAI_API_KEY`), Anthropic (`ANTHROPIC_API_KEY`) — la plupart des SDK.
- **Override** : `klef add <name> --as CUSTOM_VAR_NAME` mémorise le nom dans l'index. `klef export` et `klef run` l'utilisent ensuite automatiquement.

### 3.5 Hors-scope MVP

- Synchro multi-machines (peut-être iCloud Keychain en v0.4)
- TUI / GUI
- Linux headless, Windows
- Rotation, expiration, audit log
- Tags, catégories, recherche full-text
- Plugin VS Code

## 4. Architecture

### 4.1 Découpage des modules

```
src/
├── main.rs              # thin wrapper: parse args, call lib::run, format errors
├── lib.rs               # public crate root: declares modules + run() entrypoint
├── cli.rs               # définitions clap
├── commands/
│   ├── mod.rs
│   ├── add.rs
│   ├── get.rs           # gère "get" et "show"
│   ├── list.rs
│   ├── rm.rs
│   ├── edit.rs
│   ├── rename.rs
│   ├── export.rs
│   └── run.rs
├── store/
│   ├── mod.rs           # Store struct (combine backend + index)
│   ├── backend.rs       # trait Backend + MemoryBackend (in-process)
│   ├── keychain.rs      # KeychainBackend (crate `keyring`)
│   ├── file.rs          # FileBackend (plaintext JSON, cross-process)
│   └── index.rs         # IndexFile (lecture/écriture JSON métadonnées)
├── envfile.rs           # parser .env + résolution "klef:<name>"
└── error.rs             # KlefError unifié
```

**Pourquoi bin + lib** : la logique vit dans `lib.rs` (testable directement, accessible aux tests d'intégration), `main.rs` ne fait que parser les args et imprimer les erreurs. Permet `cargo test --lib` sur n'importe quel module et facilite la séparation des responsabilités.

**Règle** : chaque fichier non-doc reste sous 300 lignes (vérifié par `scripts/check-lines.sh` au pre-commit).

### 4.2 Trait `Backend`

```rust
pub trait Backend {
    fn get(&self, name: &str) -> Result<String, KlefError>;
    fn set(&self, name: &str, value: &str) -> Result<(), KlefError>;
    fn remove(&self, name: &str) -> Result<(), KlefError>;
}
```

- **Prod** : `KeychainBackend` qui wrap `keyring::Entry::new("klef", name)`.
- **Tests in-process** : `MemoryBackend` (HashMap protégé par Mutex). Utilisé par les tests unitaires Rust qui restent dans le même processus.
- **Tests cross-process / E2E** : `FileBackend` (JSON plaintext à un chemin temporaire). Indispensable parce que les tests `assert_cmd` lancent un nouveau process à chaque appel et `MemoryBackend` perdrait son état.
- **v0.3+** : enrobage de `FileBackend` avec chiffrement `age`. Le code existant ne change pas, on ajoute une couche de chiffrement au `set`/`get`.

**Sélection du backend au runtime** (dans `lib.rs::build_store`) :
| `KLEF_TEST_BACKEND` | Backend |
|---|---|
| absent | `KeychainBackend` |
| `file:/path/to/secrets.json` | `FileBackend(/path/to/secrets.json)` |

## 5. Stockage

### 5.1 Keychain (valeurs)

- Service : `klef`
- Account : nom de la clé
- Password : valeur

Une entrée Keychain par clé. Visible dans Keychain Access sous `klef/<name>`.

### 5.2 Index JSON (métadonnées non sensibles)

**Emplacement** :
- macOS : `~/Library/Application Support/klef/index.json`
- Linux : `${XDG_CONFIG_HOME:-~/.config}/klef/index.json`

**Schéma** :
```json
{
  "version": 1,
  "keys": {
    "stripe": {
      "env_var": "STRIPE_API_KEY",
      "note": "prod compte besle",
      "added_at": "2026-05-05T19:57:00Z",
      "updated_at": "2026-05-05T19:57:00Z"
    }
  }
}
```

**Invariants** :
- L'index liste toutes les entrées Keychain créées par klef, et seulement celles-là.
- Écriture atomique : write `index.json.tmp` + `rename` final pour éviter la corruption en cas de crash.

**Désynchros possibles** :
- Index présent, valeur absente → erreur claire à l'usage, suggestion `klef rm`.
- Valeur présente, index absent → invisible. Acceptable. `klef list --repair` envisageable plus tard.

## 6. `klef run` — résolution de références

### 6.1 Syntaxe

Toute valeur d'un `.env` qui commence exactement par `klef:` est une référence. Le reste est littéral.

```
STRIPE_KEY=klef:stripe          # référence
ANTHROPIC_KEY=klef:anthropic    # référence
DATABASE_URL=postgres://...     # littéral
PORT=3000                       # littéral
```

### 6.2 Algorithme

1. Parse le `.env` (parser maison dans `envfile.rs`).
2. Pour chaque ligne `KEY=VALUE` :
   - Si `VALUE` commence par `klef:` → `store::get(name)`. Erreur si introuvable.
   - Sinon → garder tel quel.
3. Construire la commande enfant : `Command::new(prog).args(...).envs(resolved)`.
4. Sur Unix : `std::os::unix::process::CommandExt::exec()` — klef est remplacé par la commande, pas de process zombie. C'est l'API standard pour `execvp` en Rust.

### 6.3 Détails

- **Pas de fuite** : valeurs résolues jamais écrites sur disque ni stdout. Vivent uniquement dans l'env du process enfant.
- **`exec` (Unix uniquement, OK pour le MVP)** : klef se remplace par la commande. Pas de process zombie, signaux propagés naturellement, code retour direct du shell parent. Comme le MVP cible macOS + Linux, c'est cohérent. Si Windows entre dans le scope plus tard, on ajoutera un `#[cfg(not(unix))]` qui retombe sur `Command::status()`.
- **Référence cassée** : refus de lancer la commande, exit code 3, message clair.

### 6.4 Cas limites du parser

- Lignes vides, commentaires `#` ignorés.
- Guillemets : `KEY="value"` et `KEY='value'` → strip les quotes.
- Noms avec tiret : `klef:stripe-prod` → OK.
- Pas de variable substitution (`${OTHER}`) — non supporté en v0.1, `.env` n'en a pas besoin.

### 6.5 Pourquoi un parser maison

Les libs dotenv (`dotenvy`) feraient le job, mais on veut un contrôle total sur ce qu'on considère "valeur littérale" vs "référence". 50 lignes max. Évite une dépendance externe pour une feature centrale.

## 7. Gestion des erreurs

### 7.1 Type unifié

`KlefError` (enum via `thiserror`) avec variants par catégorie :

| Catégorie | Variants |
|---|---|
| Backend | `BackendUnavailable`, `BackendDenied` |
| Storage | `IndexCorrupt`, `IndexWrite` (vraie écriture index uniquement) |
| I/O générique | `Io(io::Error)` (stdin, stdout, fichiers `.env`, etc.) |
| User | `KeyNotFound`, `KeyAlreadyExists`, `InvalidKeyName` |
| Run | `EnvFileNotFound`, `BrokenReference`, `CommandFailed` |

**Règle** : `IndexWrite` reste réservé aux échecs d'écriture/atomic-rename de l'index. Toute autre erreur I/O (lecture du `.env`, prompt, etc.) passe par `Io`. Garder ces deux variants distincts évite que le message "failed to write index" apparaisse pour des erreurs qui n'ont rien à voir avec l'index.

### 7.2 UX

- Messages en anglais (potentiel open-source).
- Chaque erreur affiche une **action concrète** à effectuer.
  - Ex. `KeyNotFound("stripe")` → `"Key 'stripe' not found. List available keys: klef list"`.
- Pas de stack trace par défaut. `KLEF_DEBUG=1` active le mode verbeux avec chaîne de causes.

### 7.3 Exit codes

| Code | Sens |
|---|---|
| 0 | OK |
| 1 | Erreur générique |
| 2 | Clé inconnue |
| 3 | Référence cassée dans `klef run` |
| 4 | Backend indisponible |
| 64 | Mauvaise utilisation CLI (convention `sysexits.h`) |

## 8. Stratégie de tests

### 8.1 Unitaires

- **`envfile.rs`** : guillemets, commentaires, lignes vides, références, cas tordus.
- **`store::index`** : sérialisation, round-trip, écriture atomique (avec `tempfile`).
- **`commands::export`** : formats `shell` et `dotenv`.

### 8.2 Intégration in-process via `MemoryBackend`

- Le `Store` consomme un `Backend`, ce qui permet d'injecter un backend de test sans toucher au Keychain réel.
- En tests Rust qui restent dans le même processus, on injecte `MemoryBackend` (HashMap+Mutex).

### 8.3 E2E CLI cross-process via `FileBackend`

- `assert_cmd` + `predicates` — lance le binaire, vérifie stdout/stderr/exit code.
- Backend fichier activé via `KLEF_TEST_BACKEND=file:/tmp/secrets.json` pour partager l'état entre plusieurs invocations du binaire.
- Couverture : workflows complets `add → list → get → rm` et `klef run` avec `.env` temporaire.

### 8.4 Pas testé en CI

- Vrai Keychain macOS : trop fragile, pollue le Keychain user. Test manuel à chaque release.

## 9. Stack technique

| Domaine | Choix | Justification |
|---|---|---|
| Langage | Rust édition 2024 | Single binary, écosystème CLI mature |
| Args parsing | `clap` (derive) | Standard de fait, ergonomique |
| Keychain | `keyring` | Cross-platform out-of-the-box |
| Prompt masqué | `rpassword` | Standard, petit |
| Sérialisation | `serde` + `serde_json` | Standard |
| Erreurs | `thiserror` | Boilerplate minimal |
| Index temps | `chrono` ou `time` | À trancher (pencher pour `time`) |
| Tests CLI | `assert_cmd` + `predicates` | Standard CLI testing |
| Lints | `clippy::pedantic` + `nursery` (warn) | Cf. `Cargo.toml` |

## 10. Roadmap après MVP

- **v0.2** : `--backend file` (age-encrypted), Windows Credential Manager.
- **v0.3** : `klef edit` riche (TUI ?), auto-complétion shell, `klef search`.
- **v0.4** : Synchro iCloud Keychain (macOS).
- **v0.5+** : plugin VS Code, hooks Git pour détecter des fuites de clés.

## 11. Décisions tranchées

| # | Question | Décision |
|---|---|---|
| 1 | Modèle de données | Plat (A) |
| 2 | UX `klef add` | Prompt masqué TTY + stdin (A+C) |
| 3 | Convention env var | `<NAME>_API_KEY` + `--as` mémorisé (D+B) |
| 4 | Sortie `klef get` | Newline TTY-aware + `klef show` séparé |
| 5 | Killer feature | `klef run` avec références `klef:<name>` |
| 6 | Plateformes MVP | macOS + Linux desktop |
| 7 | Backend Linux headless | v0.3 (fichier chiffré) |
| 8 | Commandes finales | 9 (avec `edit` et `rename`) |
| 9 | Parser `.env` | Maison (50 LOC max) |
| 10 | Langue messages | Anglais |

## 12. Questions ouvertes (à trancher pendant l'implémentation)

- **`time` vs `chrono`** pour les timestamps de l'index — décision à prendre au moment du PR sur `store::index`.
- **Format de sortie de `klef show`** — encadré ASCII, simple ligne, ou avec un label ? À tester en pratique.
- **Comportement de `klef rename`** si la nouvelle valeur existe déjà — refus par défaut, flag `--force` envisageable.
