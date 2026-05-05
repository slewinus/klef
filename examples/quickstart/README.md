# Quickstart — `klef run`

Smallest possible end-to-end demo of klef's killer feature : un `.env` qui contient des **références** plutôt que des valeurs, résolu au runtime par `klef run`.

## Setup unique

Sauvegarde une clé Stripe (factice) dans ton Keychain :

```bash
echo -n "sk_test_demo_value" | klef add stripe
```

(Ou `klef add stripe` puis colle la valeur au prompt masqué.)

## Le `.env` de ce dossier

```
STRIPE_KEY=klef:stripe          # référence — résolue au runtime
PORT=3000                       # littéral
DATABASE_URL=postgres://...     # littéral
```

Aucun secret en clair. Tu peux commit ce fichier.

## Lancer

Depuis ce dossier (`examples/quickstart/`) :

```bash
klef run -- ./demo.sh
```

Tu devrais voir :

```
── env reçu par le process enfant ──
STRIPE_KEY    = sk_test_demo_value
PORT          = 3000
DATABASE_URL  = postgres://localhost/myapp
```

`STRIPE_KEY` a été résolu depuis le Keychain. Les deux autres variables sont littérales — passées tel quel au process enfant.

## Adapter à ton vrai projet

Mêmes principes :

1. `klef add <name>` pour chaque secret.
2. Dans le `.env` du projet, remplace `SECRET=valeur_en_clair` par `SECRET=klef:<name>`.
3. Lance avec `klef run -- <ta_commande_habituelle>` (ex. `npm start`, `python app.py`, `cargo run`).

Marche avec n'importe quel langage qui lit `process.env` / `os.environ` / `std::env::var`.
