# PLÁN PROJEKTU — FERMIFORGE

*(Finální uzavírací verze, self-contained. Všechna rozhodnutí z celé diskuze jsou zafixovaná: jméno Fermiforge, slow-motion výkonový model, provenance Value/derive, měsíční cron s PR-gated updaty, URL sdílení s verzováním, kompletní round-trip test suite, noční property testy, e-INFRA/LLM disclosure, izolace AI souborů mimo git, CI konfigurace. Dokument je určen jako `PLAN.md` do kořene repa v M0 a je trvalým zdrojem pravdy pro každého přispěvatele, lidského i agentního AI.)*

---

## 1. Název

**`Fermiforge`** — "kovárna fermionů".

Jméno rezonuje napříč všemi vrstvami projektu doslova, ne jen rétoricky:
- kvarky a leptony (celý obsah projektu) **jsou fermiony**,
- HOMO/LUMO a elektronová struktura molekul = **Fermiho hladina** (přesně to, co Hückel viewer ukazuje),
- Fermiho povrch a statistika Fermi-Dirac patří do 2D Dirac/grafen vrstvy (fáze M5),
- Enrico Fermi je jednou z nejvýznamnějších postav mostu mezi částicovou, jadernou a atomovou fyzikou – tedy přesně prostupnost "od kvarků po molekuly", kterou projekt pokrývá.

**Stav průzkumu (2026):** GitHub: 0 kolizí (repo, organizace, vyhledávání). Web: jméno neexistuje. Registry (crates.io, npm, PyPI) a domény ověřit ručně v den M0 podle checklistu v § 12.

**Konvence pojmenování součástí:**
- GitHub repo: `fermiforge` (osobní účet, později případně organizace `fermiforge`)
- Core crate: `fermiforge-core` (crates.io, publikovatelný)
- Web app: `fermiforge` na GitHub Pages (`username.github.io/fermiforge`)
- Doména (až bude zájem): `fermiforge.dev` / `.io`

**Jazyk projektu: kód, commit messages, docs, issues = angličtina.** UI aplikace anglicky s připravenou lokalizací (i18n od začátku, čeština jako druhý jazyk pro vlastní potřebu).

---

## 2. Vize

**Fermiforge je webová stavebnice hmoty od kvarků po molekuly.** Uživatel skládá částice, atomy a molekuly a v reálném čase pozoruje jejich orbitaly, spektra a chování. Žádná fyzikální hodnota není zapečetěná v kódu – částice, konstanty i parametry jsou editovatelná data, takže lze zkoumat nejen skutečnou fyziku, ale i "co kdyby" scénáře (těžší elektron, jiná jemná strukturová konstanta, mionové atomy, přepsané kvarky).

Vše běží na klientovi (statický web, GitHub Pages), fyzika se počítá v Rust/WASM, vizualizace ve WebGL2 (WebGPU později jako upgrade). Aplikace nikdy nesnižuje fyzikální přesnost – na pomalejších strojích se pouze zpomalí plynutí času simulace. Libovolný stav scény lze sdílet přes URL, bez serveru a bez ukládání dat.

**Slogan pro README:** *"Build matter from quarks to molecules – and rewrite the laws while you're at it."*

---

## 3. Cíle a non-goals

### Cíle
1. Interaktivní prohlížeč molekulových orbitalů (Hückel/LCAO) s editací geometrie v reálném čase (< 5 ms přepočet).
2. Atomová vrstva: relativistické orbitaly (radiální Diracova rovnice), exotické atomy (mion, pozitronium), editace Z a konstant.
3. Skládací vrstva: kvarky → hadrony → jádra (pravidla + semiempirické modely), vše jako editovatelná data.
4. Plná provenance každé fyzikální hodnoty (zdroj, edice, nejistota) viditelná v UI.
5. Publikovatelný `fermiforge-core` crate + JOSS paper + Zenodo DOI.
6. Sdílení libovolného stavu scény přes URL (fragment, komprese, verzované schéma) bez serveru a bez ukládání – včetně čitelné query formy pro jednoduché scény.

### Non-goals (explicitně mimo rozsah)
- Žádná QCD na mřížce ani first-principles výpočty hadronů (používají se semiempirické aproximace, vždy označené).
- Žádný DFT, žádná ab-initio kvantová chemie (max. Hückel/STO-3G).
- Žádný server, žádné uživatelské účty, žádná backendová databáze.
- Žádný multiplayer ani server-side sdílení (sdílení probíhá výhradně bezstavově přes URL / export JSON).
- Žádné záměrné zjednodušování fyziky pro výkon (viz D4).

---

## 4. Architektura a struktura repa

**Klíčové rozdělení: VEŘEJNÉ vs. AI-LOKÁLNÍ.** Soubory pro agentní AI nikdy nevstupují do gitu (viz § 14 a pravidlo § 13.12).

```
fermiforge/                      # VEŘEJNÉ (v gitu)
├── PLAN.md                      # tento dokument – zdroj pravdy
├── .gitignore                   # vč. AI guard bloku (§ 14)
├── core/                        # Rust: VEŠKERÁ fyzika, nula UI kódu
│   ├── value/                   # typ Value (provenance wrapper) + derive()
│   ├── state/                   # serializace/deserializace scény, URL codec, migrace verzí
│   │   └── tests/               # round-trip testy (§ 8.6), property-based testy
│   ├── huckel/                  # LCAO/Extended Hückel engine
│   ├── dirac_atom/              # radiální Dirac, shooting method, logaritmická síť
│   ├── compose/                 # kvarky→hadrony→jádra (pravidla + semiempirika)
│   ├── schema/                  # serde typy pro data/*.json, versioning
│   └── tests/                   # validace proti analytickým řešením (§ 8)
├── data/
│   ├── particles.json           # snapshoty z ingest pipeline (immutable, versionované)
│   ├── basis_sto3g.json         # z BSE
│   ├── nuclei.json              # z IAEA/AME
│   └── README.md                # pravidla evoluce schématu (§ 7.4)
├── ingest/                      # Python: fetch → normalizace → validace → PR
│   ├── sources/                 # pdg.py, bse.py, iaea.py, codata.py
│   ├── validate.py              # "žádné holé číslo" + schema kontrola
│   └── value.py                 # Value.from_source() – sdílená definice
├── web/                         # Frontend (Svelte/TS): WASM binding, WebGL2 render, UI
├── scripts/
│   └── install-hooks.sh         # pre-commit AI guard (§ 14)
├── .github/workflows/
│   ├── ci.yml                   # ai-guard + rust + wasm + build + data-integrity (§ 10)
│   ├── nightly-prop.yml         # noční hluboké property testy (§ 8.6)
│   ├── deploy.yml               # build → GitHub Pages při push na main
│   └── data-update.yml          # MĚSÍČNÍ CRON (§ 7.3)
├── CITATION.cff
├── LICENSE                      # MIT (core), data pod licencemi zdrojů v NOTICES
├── NOTICES
└── README.md

fermiforge/.ai/                  # LOKÁLNÍ (nikdy v gitu, celé gitignorované)
├── AGENTS.md                    # operativní pravidla pro agenta (§ 14)
├── CLAUDE.md → AGENTS.md        # symlink/alias pro různé agent tooling
├── context/                     # poznámky pro agenta, odkazy na konverzace
└── fixtures-private/            # cokoli citlivého pro AI workflow
```

**Základní principy:**
- Core nezná DOM, WebGL ani networking. Má jen vstupní data + parametry → výstupní struktury. Testovatelné v čistém Rustu, publikovatelné na crates.io.
- Web je "jen" jeden z klientů jádra. (Budoucnost: Python binding přes PyO3 jako druhý klient.)
- Žádný soubor v `core/` neobsahuje konkrétní fyzikální konstantu – vše přichází z `data/`.
- PLAN.md je veřejný záměrně: lidská dokumentace strategie, cenná pro JOSS recenzenty i komunitu.

---

## 5. Klíčová technická rozhodnutí (závazná)

| # | Rozhodnutí | Důvod |
|---|---|---|
| D1 | **Rust → WASM** pro core | výkon, publikovatelnost, budoucí PyO3 |
| D2 | **WebGL2 jako default render**, WebGPU až ve fázi M5 | WebGL2 je dnes všude; WebGPU jen pro 2D Dirac sandbox |
| D3 | **Fixní simulační krok Δt, nikdy nezvětšovaný** | unitarita, deterministické výsledky napříč stroji |
| D4 | **Slow-motion místo ořezu přesnosti** | pomalý stroj = pomalejší plynutí času, stejně pravdivá fyzika; `time_scale = f(measured_perf, user_slider)`; norm ∫\|ψ\|²dV jako bezplatný indikátor přesnosti |
| D5 | Interpolace mezi simulačními stavy pro plynulý render (lerp re/im složek) | 120+ Hz monitory bez trhání; render nezávislý na simulačním taktu |
| D6 | **Data: build-time ingest, runtime offline** | CORS nestabilita API, reprodukovatelnost, offline-first |
| D7 | Immutable data + **uživatelský overlay** (IndexedDB), nikdy přímé přepsání | diff proti reálné fyzice = didaktický prvek |
| D8 | Fyzika na GPU **až** u 2D Dirac; Hückel a Dirac-atom CPU/WASM | Hückel (60×60 matice) a radiální ODE jsou na CPU triviální |
| D9 | **Sdílení stavu výhradně přes URL fragment** (`#/s=`), verzované schéma, čitelná query forma pro jednoduché scény | nulová serverová infrastruktura, fragment se nikdy neposílá na server, refresh zachovává stav |
| D10 | **AI soubory mimo git** – `.ai/` adresář, guard v CI i pre-commit hooku | AI/agent soubory nikdy neopustí lokální stroj (§ 14) |

---

## 6. Fyzikální obsah

### 6.1 Molekulová vrstva (`core/huckel`)
- Extended Hückel: překryvové integrály nad STO-3G bází, symetrický problém vlastních čísel přes Choleského rozklad S, Jacobi/QR diagonalizace.
- Výstupy: koeficienty C, energie ε, HOMO/LUMO, elektronová hustota.
- Cíl výkonu: 60-atomová molekula < 5 ms na průměrném notebooku (WASM).

### 6.2 Atomová vrstva (`core/dirac_atom`)
- Radiální Diracova rovnice, velká/malá složka P(r), Q(r); shooting method + Numerov/RK4 na logaritmické mřížce.
- Sférické spinory Ω_jlm; konečný jaderný poloměr (Fermiho model rozložení náboje) pro vysoká Z.
- Editovatelné: Z, hmotnost a náboj orbitujícího leptonu (elektron/mion/anti-e/…), α.
- Výstupy: hladiny E_nj, radiální hustoty → GPU render.

### 6.3 Skládací vrstva (`core/compose`)
- Kvarky jako data (náboj, hmotnost, spin, barva z PDG). Baryony/mesony pravidly barevné neutrality + spinového sčítání; hmotnosti přes semiempirické vazebné modely.
- Jádra: Weizsäcker; hodnoty porovnávané s IAEA/AME daty.
- **Povinné UI označení aproximace:** každá odvozená hodnota nese `method`, UI rozlišuje "vypočteno" vs. "tabulková hodnota" vs. "aproximace".

---

## 7. Datová vrstva a update pipeline

### 7.1 Typ Value (provenance)
Každá fyzikální hodnota v `data/` i ve výpočetním jádru:
```json
{ "value": 2.16, "uncertainty": 0.09,
  "source": "PDG", "edition": "2024", "fetched": "2026-02-14",
  "url": "https://pdglive.lbl.gov/...", "id": "particles/up/mass_MeV" }
```
- `Value.from_source()` – jediná cesta pro vstupní data.
- `derive(fn, inputs, method)` – jediná cesta pro odvozené hodnoty; propaguje nejistotu, slučuje provenance, ukládá `derived_from` a `method`.
- Uživatelské úpravy: overlay s `source: "user"` + `based_on` (původní edice) – jednotně zpracovávané stejným systémem.
- CI pravidlo: **žádné holé číslo** – každá číselná položka musí mít `source` + `edition` (validační skript `ingest/validate.py`).

### 7.2 Zdroje
| Vrstva | Zdroj | Frekvence |
|---|---|---|
| Částice/kvarky | PDG REST API | ročně |
| Báze STO-3G | Basis Set Exchange (JSON export) | nepravidelně |
| Jádra | IAEA LiveChart REST + AME | ročně/výjimečně |
| Konstanty | CODATA (přes `scipy.constants`) | ~4 roky |

### 7.3 Update workflow (měsíční cron)
```
cron (1. den v měsíci, GitHub Actions, data-update.yml)
 → ingest stáhne aktuální edice všech zdrojů
 → diff proti data/*.json v repu
 → beze změn → tiše skončí
 → změny → přegenerovat data (nová provenance)
          → spustit FYZIKÁLNÍ validační testy (§ 8)
          → pass → otevřít PR (peter-evans/create-pull-request)
                    s lidsky čitelným souhrnem změn v těle PR
          → fail → otevřít issue "data rozbila test X", žádný PR
```
+ `workflow_dispatch` pro ruční spuštění (např. nová AME tabulka).
Ingest skripty mají vlastní unit testy proti fixture souborům (uložené ukázky API odpovědí) – změna formátu API = explicitní selhání, nikoli tichá chyba.

### 7.4 Schema versioning
- Každý soubor má `schema_version`; evoluční pravidlo: **přidávat pole lze, měnit význam existujících nelze** (major bump + migrace). Zapsáno v `data/README.md` před prvním commitem.
- Runtime kontrola nových dat: appka při startu (online) porovná `manifest.json` → nabídne update, vše staticky z GitHub Pages.

### 7.5 Stav a sdílení přes URL
**Schéma URL:**
```
https://username.github.io/fermiforge/#/s=v1.<ENCODED>
```
- Používá se **fragment (`#`), nikdy query (`?`) pro komprimovanou formu** – fragment se neposílá na server (soukromí experimentů), nezpůsobuje reload stránky.
- Kódovací pipeline: `state (Rust struct, serde)` → kanonizované JSON (seřazené klíče, vynechané defaulty) → komprese (DEFLATE, `miniz_oxide`) → base64url bez paddingu → fragment.
- **Verzování:** `v1` prefix; dekodér umí řetěz migrací `v1→v2→v3`; neznámá verze → slušná chybová zpráva.
- **Čitelná forma pro jednoduché scény:** `#/atom?Z=82&lepton=muon` (max ~5 parametrů) – pro ruční psaní do prezentací. Kanonická komprimovaná forma pro vše složitější.
- **Rozsah a limity:** celkový stav < 6 kB po kompresi (bezpečný limit URL ~8 kB včetně messaging aplikací). Sdílí se **pouze popis scény (vstupy: geometrie, parametry, overrides), nikdy vypočtená data** (orbitaly, mřížky). Bariéry v 2D Dirac scénách jsou vektorové tvary, ne bitmapy. Pro scény nad limit: JSON export souboru.
- **UX:** tlačítko "Copy link" + do schránky; adresní řádek se aktualizuje s debounce (500 ms) → refresh zachovává stav bez IndexedDB; URL od cizince je nedůvěryhodný vstup → striktní validace + limity (max 1000 atomů, max 50 overrides atp.).
- Modul žije v `core/state/` – sdílení je funkčností jádra (použitelné i budoucím Python bindingem).
- URL sdílení je hlavní distribuční mechanismus projektu (virálnost ve výuce bez serveru) – v M1 priorita hned za rendererem.

---

## 8. Validační sady (brána kvality)

CI musí projít tyto testy před každým merge:

1. **Hückel:** spektrum benzenu vs. literární hodnoty (tolerance 0,01 β), aromaticita, správná symetrie MO.
2. **Dirac-atom:** hladiny vodíku vs. analytické Diracovo spektrum (relativistické korekce); ionizační energie lehkých prvků vs. NIST; konvergence mřížky.
3. **Compose:** náboj a hmotnost protonu vs. PDG (složení uud); vazebné energie lehkých jader vs. AME (tolerance dána modelem, dokumentovaná).
4. **Data integrity:** žádné holé číslo, provenance kompletní, `schema_version` aktuální.
5. **Determinismus:** tentýž vstup → bitově identický výstup (fixní Δt).
6. **Round-trip stavu:** detail níže.

### 8.6 Detailní struktura round-trip testů

**Skupina A – základní round-trip (M0):**
- `roundtrip_empty_scene`, `roundtrip_huckel_scene` (fixture benzen), `roundtrip_atom_scene` (muonic Pb s overrides).
- `Scene` derivuje `PartialEq`; fyzikální hodnoty kvantizované (fixed-point 0,01 Å, konzistentní zaokrouhlení), aby `assert_eq!` byl spolehlivý; float výjimky s explicitním ε a komentářem.

**Skupina B – deterministická serializace (M0, TDD před codec):**
- `serialization_is_deterministic` (dvě zakódování = identický string),
- `key_order_is_canonical` (kanonizované JSON, seřazené klíče),
- `defaults_are_omitted` (default scéna < 80 znaků – chrání invariant krátkých URL).

**Skupina C – property-based testy (M1):**
- `proptest` s `#[ignore]` (denní CI nespouští): `roundtrip_random_scenes`, `roundtrip_symmetric_in_overrides` (0–50 overrides); `Arbitrary` pro `Scene` omezené na platné scény; failing seedy do `proptest-regressions/` (commitnuté); noční workflow s `PROPTEST_CASES=100000`, denní default 256.

**Skupina D – robustnost a limity (M0):**
- `rejects_unknown_version`, `rejects_corrupted_payload` (prázdný/nesmyslný vstup nikdy nepanikaří), `rejects_oversized_state` (nad 6 kB → explicitní `TooLarge`), `url_input_is_validated_against_limits` (hostilní scéna s 99999 atomy odmítnuta).

**Skupina E – migrace (od prvního schema bumpu):**
- `migration_chain_v1_to_current`: při každém bumpu uložit fixture serializace reprezentativní scény ve staré verzi (`fixtures/scene_v1.txt`) + přidat krok do řetězce.

**Skupina F – WASM endpoint (M1):**
- `wasm_bindgen_test` v headless Chrome: parsování URL fragmentu přes Web API. CI: `wasm-pack test --headless --chrome core`.

---

## 9. Milestones

| Fáze | Obsah | Kritérium dokončení | Odhad |
|---|---|---|---|
| **M0: Skeleton** | repo, CI (ai-guard, rust, wasm, build, data-integrity – § 10), `scripts/install-hooks.sh` (pre-commit AI guard), CITATION.cff, LICENSE, `core/value` + `ingest/value.py`, `core/state/` (schéma v0 + URL codec, testy skupin A, B, D), schema dat v0, první `data/` snapshot, `.ai/AGENTS.md` lokálně, **test guardu: záměrný pokus o commit AGENTS.md musí selhat lokálně i v CI**, ruční ověření registrů (§ 12) | CI zelené na prázdném core, data se regenerují z ingestu, round-trip testy pro prázdnou scénu prochází, guard funkční | 1–2 týdny |
| **M1: Hückel MVP** | `core/huckel` + validační testy (benzen), minimální WebGL2 renderer orbitalů, editace geometrie (tahání atomů), overlay + diff UI, URL sdílení scény, proptest skupiny C + F | uživatel postaví C₆H₆ a vidí HOMO/LUMO v reálném čase; copy link → načtení → refresh zachovává stav; testy § 8.1 a § 8.6 zelené | 4–8 týdnů |
| **M2: Publikace v1** | crates.io publish `fermiforge-core`, Zenodo release + DOI, README + docs (včetně e-INFRA acknowledgmentu, § 15), příprava JOSS submission (včetně LLM disclosure, § 15) | crate stažitelný, DOI funkční, JOSS draft odeslán | 2 týdny po M1 |
| **M3: Atomy** | `core/dirac_atom`, orbital viewer, exotické atomy, editace Z/konstant, propagace α do celé appky, URL sdílení atomových scén (čitelná forma) | testy § 8.2 zelené; přepnutí elektron→mion < 10 ms; `#/atom?Z=82&lepton=muon` funguje | 6–10 týdnů |
| **M4: Compose** | kvarky jako data, hadrony, jádra, Weizsäcker, provenance propagation přes `derive()` | testy § 8.3 zelené | 4–6 týdnů |
| **M5: 2D Dirac sandbox** | split-step Fourier, WebGL2 render, kreslení bariér (vektorově), Kleinovo tunelování, slow-motion/time_scale (D4, D5), WebGPU upgrade cesta, URL sdílení Dirac scén | transmisní test vs. analytická Kleinova formule; deterministický test napříč zařízeními | 8–12 týdnů |

**Pravidlo:** mezi fázemi se nepostupuje, dokud předchozí nemá zelená kritéria. M2 je výjimečně brzká záměrně – publikovatelný artefakt vzniká po první smysluplné vrstvě, ne na konci.

---

## 10. CI/CD a release strategie

### 10.1 Workflow přehled

| Workflow | Trigger | Účel |
|---|---|---|
| `ci.yml` – `ai-guard` | každý PR/push | fail-fast guard proti AI souborům, samostatný job |
| `ci.yml` – `rust-tests` | každý PR/push | fmt, clippy (`-D warnings`), nativní testy (round-trip skupiny A, B, D, E) |
| `ci.yml` – `wasm-tests` | každý PR/push | skupina F, headless Chrome |
| `ci.yml` – `build` | každý PR/push | release build + size budget (< 5 MB warning) |
| `ci.yml` – `data-integrity` | každý PR/push | "žádné holé číslo", schema verze |
| `nightly-prop.yml` | cron 03:00 UTC / ručně | 100k property caseů (`--ignored`), failing seedy jako artifact |
| `data-update.yml` | cron 1. den v měsíci / ručně | ingest → validace → PR/issue (§ 7.3) |
| `deploy.yml` | push na main | build → GitHub Pages |

### 10.2 Klíčové technické detaily

- **AI guard job (v `ci.yml`):** `git ls-files` + grep na `AGENTS.md`, `CLAUDE.md`, `.aider*`, `.cursorrules`, `GEMINI.md`, `.ai/`, `.cursor/`, `context/`; warning, pokud soubory existují na disku netracknuté. Samostatný job s `fetch-depth: 0`.
- **Verze `wasm-bindgen-cli`** musí odpovídat verzi crate v Cargo.toml – v CI řešit dynamicky přes `cargo tree -p fermiforge-core -i wasm-bindgen` (nejčastější zdroj kryptických selhání). Fallback pro Chrome problémy: `--firefox` s geckodriver.
- **Pre-commit hook (lokální):** `scripts/install-hooks.sh` instaluje zrcadlo CI guardu kontrolující staged soubory (`git diff --cached --name-only`) – chytá i `git add -f` před commitem. Spouštět jednou po klonu (Development setup v README).
- **Permissions hardening:** každá workflow s minimem práv (`contents: read`; jen `deploy.yml` a `data-update.yml` potřebují `contents: write`).
- **Noční property testy:** testy s `#[ignore]`, `timeout-minutes: 30`, failing seedy uploadované jako artifact (noční job nemá push práva – seedy do `proptest-regressions/` přenáší člověk/agent v PR s fixem).

### 10.3 Release

- git tag `vX.Y.Z` → crates.io publish + GitHub release + Zenodo verze (koncept DOI ukazuje vždy na poslední).
- **CITATION.cff:** software citation pro repo + zvlášť data (data snapshoty citovatelné zvlášť).

---

## 11. Publikační a komunikační strategie

1. **JOSS paper** po M2 (scope: `fermiforge-core` + data pipeline s provenance – ingest+validace+provenance je softwarovým přínosem, ne "jen další viewer"). Součástí submission: e-INFRA acknowledgment + LLM disclosure dle § 15.
2. Reddit r/physics, r/chemistry, r/compsci showcase po M1 – vizuální demo videa + **sdílené odkazy na konkrétní scény** (nejlepší virální kanál: "klikni a uvidíš").
3. Později volitelně didaktická studie (*J. Chem. Educ.* / *Computers & Education*) – vyžaduje testování se studenty, až po M4.
4. ORCID + GitHub propojení před prvním commitem.

---

## 12. Rizika a mitigace

| Riziko | Pravděpodobnost | Mitigace |
|---|---|---|
| Přerostlý scope (klasické selhání) | vysoká | non-goals § 3 + pořadí fází; nic mimo M1 před M1 |
| API zdrojů změní formát | střední | ingest testy proti fixture souborům; fail → issue, ne tichá chyba |
| Méně času kvůli dokončení bakaláře | vysoká | M2 je záložní cíl "mám publikovatelný artefakt"; projekt nevyžaduje kontinuitu |
| Kolize názvu v registrech | nízká | ruční checklist v den M0: crates.io / npm / PyPI search `fermiforge`, Google `"Fermiforge"` v uvozovkách, GitHub org `fermiforge`, doména `fermiforge.dev`; fallback: `Nucleoforge` |
| Neustálé přepisování schématu dat/stavu | střední | § 7.4 a § 7.5 evoluční pravidla zavedena hned v M0 |
| URL přeteče limit (velké scény) | střední | limit 6 kB vynucený validací; JSON export souboru jako fallback |
| AI soubor náhodou commitnut | střední | tři vrstvy: `.gitignore` blok + pre-commit hook + CI ai-guard; guard testován záměrným selháním v M0 |
| Zastaralý e-INFRA acknowledgment text | střední | ověřit znění před M2 proti aktuálním podmínkám e-INFRA CZ (§ 15) |
| Závislost na jedné osobě (bus factor) | jistá (M0) | PLAN.md + `.ai/AGENTS.md` = plná dokumentace rozhodnutí; agentní AI může pokračovat |

---

## 13. Pravidla pro agentní AI

1. **Dodržuj fáze.** Neimplementuj nic z M3+, dokud není splněno kritérium aktuální fáze. Aktualizuj "Milestone guard" v `.ai/AGENTS.md` při přechodu.
2. **Core neobsahuje UI kód ani konstanty.** Jakákoli fyzikální hodnota v `core/` mimo test fixtures = chyba.
3. **Každá číselná fyzikální hodnota jde přes `Value`.** Holý float ve veřejném API core = chyba.
4. **Fixní Δt je nedotknutelný.** Žádná optimalizace nesmí zvětšovat simulační krok ani měnit výsledky (deterministický test to kontroluje).
5. **Přesnost > rychlost.** Nikdy nepřidávat aproximaci pro výkon bez explicitního `method` označení; slowdown je preferovaný před zjednodušením fyziky.
6. **Testy proti analytickým řešením jsou povinnou součástí každé fyzikální funkce** – žádná nová fyzika bez alespoň jednoho testu proti známému výsledku.
7. Commit messages konvenční commits, anglicky; každý PR max. jedna logická změna.
8. **Při nejistotě ve fyzice: zeptat se / zdokumentovat, ne vymyslet.** Aproximace musí být vždy označena `method` + odkaz na literaturu.
9. **Stav se serializuje jen přes verzované schéma stavu v `core/state/`.** Nová pole do scény vyžadují bump verze + migraci + fixture. Scéna obsahuje pouze vstupy (geometrie, parametry, overrides), nikdy vypočtená data (orbitaly, mřížky). Každý field má ověřený limit velikosti (celkový stav < 6 kB po kompresi). URL vstup je vždy nedůvěryhodný – validovat proti limitům, žádné výchozí důvěřování.
10. **Serializace je deterministická** (seřazené klíče, vynechané defaulty) – jinak selže round-trip i deterministický test.
11. **Konflikt pravidel či dokumentace: vítězí toto PLAN.md.** Návrh změny PLAN.md pouze přes PR.
12. **Soubory pro AI nikdy nesmí vstoupit do git.** AGENTS.md, CLAUDE.md, `.ai/` a podobné žijí výhradně v `.ai/` (gitignored, mimo git). Agent nikdy nespouští `git add` na tyto cesty; guard v CI i pre-commit hook porušení testují. Agent pracuje s PLAN.md z repa (veřejný) a vlastními instrukcemi z `.ai/` (lokální).

---

## 14. AI soubory: izolace a AGENTS.md

### 14.1 Pravidlo izolace

**Soubory pro agentní AI nikdy neopustí lokální stroj.** `.gitignore` samo o sobě nestačí (nepřekrývá `git add -f` ani historii) – proto tři vrstvy obrany:

1. **`.gitignore` blok** (na začátku souboru):
```gitignore
# === AI/agent files — NEVER commit ===
.ai/
AGENTS.md
CLAUDE.md
.aider*
.cursor/
.cursorrules
context/
# ====================================
```
2. **Pre-commit hook** (`scripts/install-hooks.sh`): kontrola staged souborů (`git diff --cached --name-only --diff-filter=ACR` + grep na zakázané vzory) – chytá i `git add -f` před vznikem commitu.
3. **CI `ai-guard` job** (§ 10): `git ls-files` kontrola; selhání = červený build.

**V M0 se guard testuje záměrným selháním:** jednou commitni AGENTS.md do testovací větve a ověř, že lokální hook i CI job selžou. Guard, který nikdy nebyl spuštěn na selhání, je jen dekorace.

### 14.2 AGENTS.md (šablona – vytvořit v M0 do `.ai/`, nikdy do kořene)

> Účel: `AGENTS.md` je operativní instrukce pro agentní AI pracující v repu. Na rozdíl od `PLAN.md` (strategie, veřejný) obsahuje každodenní pracovní pravidla. Žije výhradně v `.ai/AGENTS.md`.

```markdown
# AGENTS.md – Fermiforge agent instructions

## Project overview
Webová stavebnice hmoty od kvarků po molekuly; fyzika jako editovatelná data,
Rust/WASM core, WebGL2 render, vše na klientovi (GitHub Pages).
Viz PLAN.md § 2. NIKDY nepřevyprávěj vizi vlastními slovy – cituj PLAN.md.

## Absolute rules (non-negotiable)
1. Core neobsahuje UI kód ani fyzikální konstanty (PLAN.md § 13.2–13.3).
2. Každá číselná fyzikální hodnota jde přes typ `Value` (provenance).
3. Fixní Δt nikdy nezvětšovat (PLAN.md § 13.4).
4. Nová fyzika vyžaduje test proti analytickému řešení (PLAN.md § 13.6).
5. Žádné aproximace bez `method` označení + literárního odkazu.
6. Serializace scény jen přes verzované schéma `core/state/` (PLAN.md § 13.9).
7. URL = nedůvěryhodný vstup, vždy validovat (limity, verze).
8. Commit messages: Conventional Commits, anglicky.
9. Jeden PR = jedna logická změna.
10. Při nejistotě ve fyzice: položit otázku / zdokumentovat předpoklad,
    nikdy nevymýšlet.
11. AI soubory (včetně tohoto) nikdy ne-commitovat (PLAN.md § 13.12).

## Repository map
- `core/` – Rust, veškerá fyzika. Žádný DOM, WebGL, síť.
- `data/` – immutable JSON snapshoty z ingestu. Nikdy ručně neupravovat
  (jen ingest pipeline).
- `ingest/` – Python skripty (build-time only).
- `web/` – Svelte/TS frontend. Jediné místo s UI a WebGL kódem.
- `PLAN.md` – zdroj pravdy pro strategii. Při konfliktu vždy vítězí PLAN.md.
- `.ai/` (lokální, negitované) – tento soubor a agentí kontext.

## Working conventions
- Rust: edition 2024, clippy bez warningů, rustfmt výchozí.
- Python (ingest): ruff + pytest, Python 3.11+.
- Testy: nová funkce bez testu = nehotovo. Fyzikální funkce bez testu proti
  známému výsledku = chyba.
- CI musí být zelené před merge – žádné výjimky, žádné "fixnu pak".
- Po klonu vždy spustit `scripts/install-hooks.sh` (pre-commit AI guard).

## Milestone guard
Aktuální fáze: [M0] ← aktualizovat při přechodu dle kritérií PLAN.md § 9.
Neimplementovat nic z vyšší fáze, dokud kritéria § 9 nejsou splněna.

## When unsure
- Fyzikální nejistota → otevři issue s otázkou, neimplementuj.
- Kolize s PLAN.md → PLAN.md vítězí; navrhni úpravu PLAN.md přes PR.
- Nová fyzikální metoda → nejprve literární reference do `method` pole,
  pak implementace.
```

---

## 15. AI asistence, e-INFRA a citace

### 15.1 e-INFRA CZ acknowledgment

Při publikaci (JOSS, Zenodo, README) uvádět standardní acknowledgment české e-infrastruktury:

> *This work was supported by the Ministry of Education, Youth and Sports of the Czech Republic through the e-INFRA CZ (ID: 90140).*

Umístění: JOSS paper (sekce Acknowledgments), README, metadata Zenodo releasu.
⚠️ Přesné znění a podmínky (zejména pro LLM endpointy) ověřit před M2 proti aktuálním zdrojům e-INFRA CZ – instituce si formulace aktualizují.

### 15.2 Disclosure využití LLM

V souladu s praxí COPE/ICMJE pro AI asistenci deklarovat v Acknowledgments, co přesně LLM dělalo:

> *Parts of the source code and documentation were developed with the assistance of a large language model hosted on the e-INFRA CZ infrastructure. All physics was independently validated against analytical solutions (see the validation suite in the repository); the authors take full responsibility for the content.*

Klíčové propojení: disclosure stojí na tom, co projekt stejně dělá – validační testy proti analytickým řešením (§ 8) jsou nezávislá kontrola všeho, co LLM vygenerovalo.

**Pravidla:**
- Nikdy netvrdit, že LLM "validovalo" fyziku – validuje test suite. Toto rozlišení je důležité v paperu i v diskusi s recenzenty.
- CITATION.cff zůstává lidská (autoři, ORCID, DOI); LLM se v citaci neuvádí jako autor, jen v textových acknowledgments.

---

## 16. Postup pro založení (M0, den 0)

1. **Ověř registry a domény** (checklist § 12): crates.io, npm, PyPI search `fermiforge`; Google `"Fermiforge"` v uvozovkách; GitHub org `fermiforge`; doména `fermiforge.dev`.
2. **Založ repo `fermiforge`. První commit:** `PLAN.md` (tento dokument) + minimální `README.md` (jedna věta + slogan) + `.gitignore` (s AI guard blokem) + `scripts/install-hooks.sh`.
3. **Lokálně vytvoř `.ai/AGENTS.md`** (šablona § 14.2) – nikdy do gitu.
4. **Propoj ORCID s GitHub účtem.**
5. **Spusť `scripts/install-hooks.sh`** a otestuj guard záměrným pokusem o commit AGENTS.md (musí selhat).
6. **Teprve potom pusť agenta na M0** dle § 9 (včetně CI workflows z § 10, `core/value`, `core/state` se skupinami A, B, D, ingest pipeline a prvního data snapshotu).

---

*Tímto je plán finální a uzavřený. Všechna rozhodnutí z celé konverzace – jméno Fermiforge, slow-motion výkonový model, provenance Value/derive, měsíční cron s PR-gated updaty, URL sdílení s verzováním a čitelnou formou, kompletní round-trip test suite s nočními property testy, izolace AI souborů s třívrstvou obranou, e-INFRA/LLM disclosure, CI konfigurace včetně ai-guard – jsou v něm zafixovaná a vzájemně provázaná.*