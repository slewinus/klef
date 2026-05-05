# Roadmap — klef

## Étape 0 — Cadrage (en cours)
- [x] Nommer le projet
- [x] Rédiger README + SPEC
- [ ] Choisir le langage (Rust ou Go)
- [ ] Valider les commandes MVP avec quelques cas d'usage réels

## Étape 1 — MVP CLI (v0.1)
Objectif : pouvoir ajouter, lister, récupérer une clé via Keychain macOS.

- [ ] Init projet (cargo / go mod)
- [ ] `klef add <name>` (lecture stdin, écriture Keychain)
- [ ] `klef get <name>`
- [ ] `klef list` (lecture index JSON)
- [ ] `klef rm <name>`
- [ ] Tests manuels sur 3-4 clés réelles

## Étape 2 — Export shell (v0.2)
- [ ] `klef export <name>...` au format `export VAR=value`
- [ ] Option `--var` pour surcharger le nom
- [ ] Option `--format dotenv` pour générer un `.env`

## Étape 3 — Confort (v0.3)
- [ ] Auto-complétion (zsh / bash)
- [ ] `klef edit <name>` (modifier note ou valeur)
- [ ] `klef search <pattern>`
- [ ] Couleurs / formatage propre

## Étape 4 — Distribution
- [ ] Homebrew tap perso
- [ ] Binaire signé (notarization macOS)
- [ ] README avec gif de démo

## Plus tard (peut-être jamais)
- TUI interactive (ratatui / bubbletea)
- Support Linux / Windows
- Synchro iCloud
- Plugin VS Code
