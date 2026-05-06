# klef-gui — macOS GUI App Design

**Date :** 2026-05-06
**Statut :** Validé pour implémentation
**Périmètre :** Application macOS native (Tauri) qui consomme `klef-core` pour offrir une interface graphique au gestionnaire de secrets. Tracking issue : [#18](https://github.com/slewinus/klef/issues/18).

---

## 1. Vision en une phrase

Une app menu bar macOS qui rend klef accessible aux développeurs qui ne veulent pas taper `klef get stripe` à chaque fois ni à leurs collègues non-CLI, tout en gardant la promesse local-first (zéro cloud, OS keychain par défaut).

## 2. Audience cible

| Persona | Use case | Friction CLI | Bénéfice GUI |
|---|---|---|---|
| **Dev senior CLI-confort** (Oscar) | Cmd+Shift+K → tape "stripe" → Enter copie → paste dans VS Code | Switch terminal, taper la commande, copier la sortie | Plus rapide que le terminal pour le copy-paste fréquent |
| **Dev junior** | Browse les clés du projet, voir lesquelles existent | Faut connaître la commande exacte, mémoriser les noms | Liste visuelle, search fuzzy |
| **PM / non-tech d'une startup** | Stocker les credentials d'une intégration sans toucher à un terminal | Inaccessible | Forms classiques, drag-drop |
| **Auditeur sécurité** | Voir d'un coup d'œil la liste des secrets, leur dernière modification | Faut piper plusieurs commandes | Tableau triable + filtres |

La GUI ne remplace pas le CLI — c'est un **frontal supplémentaire** sur le même backend (`klef-core`).

## 3. Plateformes

**v1 : macOS uniquement (>= 13.0 / Ventura).**

- Le killer feature (menu bar app) brille spécifiquement sur macOS.
- Apple Developer cert disponible côté Oscar → distribution signée/notarisée propre.
- Linux / Windows : à reconsidérer si demande utilisateur. Tauri permet le port avec friction modérée mais pas zéro (icône menu bar = tray sur Linux, layout différent).

## 4. Architecture

### 4.1 Monorepo cargo workspace

Refactor du repo actuel :

```
klef/
├── Cargo.toml                  # workspace manifest
├── crates/
│   ├── klef-core/              # lib (Store, Backend, MetaStore, age, etc.)
│   │   └── src/lib.rs          # = src/lib.rs actuel
│   ├── klef-cli/               # binaire CLI actuel
│   │   ├── src/main.rs         # = src/main.rs actuel (importe klef-core)
│   │   └── src/cli.rs          # clap derive
│   └── klef-gui/               # NOUVEAU : Tauri app
│       ├── tauri.conf.json
│       ├── src/                # backend Tauri (Rust, importe klef-core)
│       └── ui/                 # frontend (Svelte ou TS vanilla)
├── examples/                   # quickstart inchangé
├── docs/                       # design + plans + ai docs inchangés
├── homebrew/                   # formule Homebrew (CLI)
├── scripts/                    # release scripts
└── tests/                      # tests d'intégration partagés
```

`klef-core` exporte tout ce qui n'est pas argument parsing : `Store`, `KeychainBackend`, `AgeBackend`, `MetaStore`, `IndexFile`, `KlefError`, `envfile::parse`. Les commandes (`add`, `get`, …) restent dans `klef-cli` parce qu'elles font du I/O TTY.

La GUI réimplémente sa propre logique de "ajouter une clé" / "lister les clés" en appelant `klef-core::Store::add(...)` etc.

### 4.2 Pourquoi Tauri (pas Swift / Egui / Iced)

| | Tauri | Swift natif | Egui / Iced |
|---|---|---|---|
| Réutilise `klef-core` directement | ✅ | ❌ (FFI à coder) | ✅ |
| UI moderne (animations, themes) | ✅ (web) | ✅ | ⚠️ (basique) |
| Single binary signé/notarisé | ✅ | ✅ | ✅ |
| Cross-platform plus tard | ✅ | ❌ macOS only | ✅ |
| Menu bar / popover | ✅ (`tauri-plugin-positioner`) | ✅ | ⚠️ (à hand-coder) |
| Taille du binaire | ~10MB | ~5MB | ~15MB |
| Maintenance dans 2 ans | Moyen | Élevé (Swift evolue vite) | Faible (lib stable) |

**Choix : Tauri**. Réutilisation maximum de Rust, UI ergonomique, porte ouverte à Linux/Windows. La perte de "100% native" sur macOS est acceptable — l'app est petite, pas un outil graphique pro.

### 4.3 Stack frontend

**Svelte 5 + Vite + TypeScript.**

- Réactivité fine-grained (les keys/tags changent souvent → re-render léger).
- Petit bundle (Svelte compile vers JS minimal, contrairement à React qui ship le runtime).
- Syntaxe lisible pour qui n'est pas dev frontend principal.

Composants UI : **shadcn-svelte** (équivalent Svelte de shadcn/ui, basé sur Tailwind). Joli par défaut, pas de design system custom à maintenir.

### 4.4 Communication frontend ↔ backend

Tauri commands en Rust exposées au frontend via `invoke()`. Format :

```rust
#[tauri::command]
fn list_keys(state: State<AppState>) -> Result<Vec<KeyDto>, String> {
    state.store.list().map(|entries| entries.into_iter().map(KeyDto::from).collect())
        .map_err(|e| e.to_string())
}
```

DTOs propres pour la sérialisation (pas de leakage des types internes via tauri-bindgen).

## 5. UI/UX

### 5.1 Mode menu bar (par défaut)

Icône dans la barre de menu macOS (à côté de wifi/batterie). Clic = popover ancré sous l'icône.

**Anatomie du popover (~400×500 px) :**

```
┌────────────────────────────────────────────┐
│  🔑 klef                    [+] [⚙]        │
│  ┌──────────────────────────────────────┐  │
│  │ 🔍 Search keys...                    │  │
│  └──────────────────────────────────────┘  │
├────────────────┬───────────────────────────┤
│ ▾ Projects     │  stripe-prod              │
│   • aviosphere │  api · billing · prod     │
│     stripe-pro │  Last used: 2 hours ago   │
│     resend     │                           │
│   • dahouse    │  [Copy value]  [Edit]    │
│     stripe     │                           │
│   • untagged   │                           │
│ ▾ Tags         │                           │
│   api (8)      │                           │
│   billing (3)  │                           │
└────────────────┴───────────────────────────┘
```

- **Sidebar (gauche)** : tree des projets + liste plate des tags non-projets. Clic filtre la liste de droite.
- **Liste centrale** : keys du contexte sélectionné, triées par dernière utilisation (à défaut, ordre alpha).
- **Détail (right pane ou modal)** : sur sélection, affiche meta + boutons d'action.

### 5.2 Convention "projet" — pas de schéma

Les projets sont des **tags préfixés `project:`**. Exemple : `project:aviosphere`, `project:dahouse`.

- Aucun changement à `KeyMeta`. Les projets sont une **vue logique** côté GUI.
- CLI continue de gérer ces tags comme n'importe quel autre tag.
- Side benefit : `klef list --tag project:aviosphere` marche déjà aujourd'hui.

Quand l'utilisateur clique "+ Add Project" dans la GUI, ça ouvre une boîte de dialogue qui crée le tag `project:nom-saisi` et le pré-applique à toute clé ajoutée dans cette section.

### 5.3 Global hotkey

`Cmd+Shift+K` invoque le popover sans avoir à cliquer l'icône. Configurable dans Settings.

Implémentation : `tauri-plugin-global-shortcut`.

### 5.4 Actions principales

| Action | Geste | Backend call |
|---|---|---|
| Voir liste | Ouvrir popover | `Store::list()` |
| Filtrer par projet | Clic sur projet sidebar | filtre côté frontend |
| Search fuzzy | Tape dans search bar | filtre côté frontend (fuzzysort.js) |
| Copier value | Clic sur key OU Enter sur sélection | `Store::get_value()` + Tauri clipboard |
| Voir value | Clic sur l'œil | `Store::get_value()` (pas auto-affiché) |
| Add key | Bouton `+` ou Cmd+N | Form modal → `Store::add()` |
| Edit | Bouton edit OU double-clic | Form modal → `Store::set_tags()` / `Store::add(force=true)` |
| Delete | Menu contextuel OU Backspace + confirm | `Store::remove()` |
| Drag-drop .env | Drag fichier sur popover | `klef discover` interactif (plan + confirm) |

### 5.5 Backend selection

**Auto par défaut** :
- App settings persistés dans `~/Library/Application Support/klef-gui/settings.json` (path Tauri standard).
- Premier lancement : utilise `KeychainBackend` (recommandé pour macOS).
- Settings → Backend onglet : option "Switch to encrypted file backend (.age)" pour les paranos / le headless local.

**Pas d'auto-détection magique de fichier `.age`** — l'utilisateur doit choisir explicitement. Ça évite les surprises ("j'ai créé un fichier age.age sur mon Desktop, l'app a switché toute seule").

## 6. Features par sprint

Chaque sprint = une PR mergeable indépendamment, app reste fonctionnelle entre.

### S1 — Workspace refactor (4h)

**Livrable** : monorepo cargo, CLI tourne pareil qu'avant, tous les tests passent.

- [ ] Créer `crates/klef-core/`, déplacer `src/lib.rs` + ses modules dedans.
- [ ] Créer `crates/klef-cli/`, déplacer `src/main.rs` + `src/cli.rs` dedans, importer `klef-core`.
- [ ] `Cargo.toml` racine devient un workspace.
- [ ] Réajuster les imports : `crate::error::KlefError` → `klef_core::error::KlefError`.
- [ ] Vérifier : `cargo build`, `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Vérifier : `klef --version`, `klef list`, `klef --backend age:...` marchent toujours.
- [ ] Mettre à jour `release.yml` pour build le bin depuis le workspace.

**Tests à ne pas casser** : 161+ existants.

### S2 — Tauri scaffold + minimal viewer (4h)

**Livrable** : app qui s'ouvre, liste les clés du Keychain, copie au clic.

- [ ] `cargo install create-tauri-app && cd crates && cargo create-tauri-app klef-gui --template svelte-ts`
- [ ] Ajouter dep `klef-core` au `Cargo.toml` du gui.
- [ ] Tauri command `list_keys()` qui appelle `Store::list()` et retourne `Vec<KeyDto>`.
- [ ] Frontend Svelte : page unique avec la liste (table simple, pas encore de design).
- [ ] Bouton "Copy" par row, copy via `tauri-plugin-clipboard-manager`.
- [ ] Build + run : `cargo tauri dev`.

### S3 — Menu bar + global hotkey (3h)

**Livrable** : icône menu bar avec popover, Cmd+Shift+K l'invoque.

- [ ] Configurer `tauri.conf.json` pour mode `tray-only` (pas de fenêtre principale).
- [ ] Icône menu bar avec template image (tinte automatiquement light/dark mode).
- [ ] Popover via `tauri-plugin-positioner` ancré sous l'icône.
- [ ] Global shortcut Cmd+Shift+K via `tauri-plugin-global-shortcut`.
- [ ] Auto-hide quand le popover perd focus.

### S4 — Search + projects sidebar (4h)

**Livrable** : sidebar avec tree des projets (tags `project:*`), search fuzzy.

- [ ] Tauri command `list_tags_with_counts()` (existe déjà côté Store).
- [ ] Frontend : composant Sidebar qui groupe les tags `project:*` au-dessus des autres.
- [ ] Click sur projet → filtre la liste centrale.
- [ ] Search bar avec fuzzysort.js, filtre live sur name + note + tags.
- [ ] Tri par "last used" — nécessite d'ajouter un `last_used_at: Option<OffsetDateTime>` au `KeyMeta` (à incrémenter sur `Store::get_value()`). **Décision** : on ajoute ce champ. Forward-compat via `#[serde(default)]`.

### S5 — Add / edit / delete forms (4h)

**Livrable** : ajouter, modifier, supprimer une clé via formulaire.

- [ ] Modal "Add Key" : champs name, value (masqué par défaut), env_var override, note, tags multi-select.
- [ ] Modal "Edit Key" : pré-rempli, peut éditer la value.
- [ ] Confirm dialog "Delete Key" avant `Store::remove()`.
- [ ] Validation côté frontend (nom non vide, format clef valide), erreur friendly si le backend rejette.

### S6 — Drag-drop .env import (3h)

**Livrable** : glisser un fichier `.env` sur la fenêtre déclenche un wizard d'import visuel.

- [ ] Tauri file drop event handler.
- [ ] Wizard 2 étapes :
  1. Aperçu (similaire à `klef import --dry-run`) : table des keys détectées avec colonne "import / skip / merge".
  2. Confirmation et exécution → `Store::add(force=true)` pour chaque ligne sélectionnée.
- [ ] Option "rewrite source file with klef: references" comme sur le CLI.

### S7 — Polish + signing/notarize (1 jour)

**Livrable** : DMG signé/notarisé, prêt à distribuer.

- [ ] Iconographie : klef logo (générer un set d'icons via [icon.kitchen](https://icon.kitchen) à partir d'un SVG).
- [ ] Light/dark mode auto.
- [ ] Theming via Tailwind tokens.
- [ ] Empty states (pas de clés, pas de search results, etc.).
- [ ] Settings panel : backend selection, theme, hotkey customization, "open at login" toggle.
- [ ] About dialog : version, link GitHub.
- [ ] **Signing/notarization** :
  - [ ] Apple Developer cert installé.
  - [ ] `tauri-plugin-updater` configuré (auto-update via GitHub Releases).
  - [ ] Build : `cargo tauri build --target aarch64-apple-darwin --target x86_64-apple-darwin`.
  - [ ] Sign + notarize via `xcrun notarytool`.
  - [ ] Universal DMG.
- [ ] Workflow GitHub Actions `release-gui.yml` qui build sur tag `gui-v*`.

## 7. Distribution

**Channels** :

1. **GitHub Releases** : DMG universel (Intel + ARM) signé/notarisé.
2. **Homebrew cask** : `slewinus/tap/klef-gui` séparé du formula CLI. `brew install --cask klef-gui`.
3. **Pas d'App Store** v1 — sandbox restrictions feraient mal au fonctionnement (accès Keychain hors entitlement standard).

**Versioning** : tags séparés `gui-v0.1.0`, etc. Le CLI garde son propre cycle (`v0.4.0`). Workspace cargo permet versions distinctes par crate.

**Auto-update** : `tauri-plugin-updater` pointe vers GitHub Releases. Vérification au démarrage + bouton manuel dans Settings → About.

## 8. Sécurité — différences vs CLI

- **Clipboard timing** : la GUI peut auto-clear le presse-papier après N secondes (configurable, default 30s). Le CLI ne peut pas, parce que pas de daemon. Ça résout naturellement [#25 reste — clipboard helper](https://github.com/slewinus/klef/issues/25).
- **Display masking** : la valeur est cachée par défaut (****), reveal explicite via clic-œil. Empêche les screenshots accidentels.
- **No persistence of decrypted values** : pour le backend age, la passphrase est gardée en mémoire pendant la session (process-lifetime). Quand l'app quitte ou se met en veille >5 min, la passphrase est wipe et il faut la retaper. Tauri permet hook sur l'événement system sleep.
- **Audit log local** : option à activer dans Settings — log de chaque accès dans `~/Library/Application Support/klef-gui/audit.log`.

## 9. Décisions ouvertes (à trancher pendant l'implémentation)

1. **Stockage du `last_used_at`** — on l'ajoute à `KeyMeta` ou on le garde en GUI-side state (pas synchronisé entre machines) ? Probablement dans `KeyMeta` pour cohérence avec le CLI.
2. **Multi-window** — est-ce qu'on veut une window de "vault management" plein écran en plus du popover, ou tout dans le popover ? Pas critique v1, popover-only pour démarrer.
3. **Recherche par tag combinée** — `project:aviosphere AND tag:billing` ? Probablement filter combinés via UI sidebar (clic projet + clic tag), pas de query language v1.
4. **Notifications système** — sur copie de value, est-ce qu'on notifie "Stripe key copied to clipboard" ? Bonus, pas v1.

## 10. Roadmap après v0.1 GUI

- **Linux / Windows ports** — Tauri rend ça abordable, demande utilisateur dépendant.
- **Sync iCloud Keychain** ([#13](https://github.com/slewinus/klef/issues/13)) — devient particulièrement intéressant avec une GUI multi-machine.
- **Browser extension** companion — extraire automatiquement les `klef:` references des `.env` ouverts dans VS Code Web. Future.
- **Touch ID per-key reveal** — option par clé "require Touch ID to reveal". Ajoute un coût UX mais protection contre vol d'écran.

## 11. Risques + mitigations

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Tauri plugin global-shortcut crash sur macOS 14+ | Faible | Moyen | Test sur tes Mac avant ship ; fallback sur menu bar click |
| Apple notarization rejette une dep | Moyen | Élevé (no-ship) | Notarize tôt (S2 idéalement), pas en bout de chaîne |
| Bundle size > 50MB | Moyen | Faible (sécurité reputational) | Mesurer dès S2 ; strip + LTO + tauri-bundler trim |
| Keychain access prompts spamment l'user | Élevé sur la prem session | Moyen (UX) | Passphrase cache + claire docs ; éventuellement issue dédiée pour ACL |
| GUI revele des bugs cachés du CLI | Moyen | Élevé (bonne nouvelle en fait) | Embrasser, fixer, on devient meilleur |

## 12. Out of scope explicite

- Cloud sync via service propriétaire.
- Multi-utilisateur (équipe partagée). Local-first par design ; partage via age recipients (futur).
- Migration depuis 1Password / Dashlane / Bitwarden. Différentes UX, scope énorme.
- Plugin VS Code pour résoudre les `klef:` refs in-editor. Future, pas v1.
- Versioning des secrets (rollback historique). Future.
