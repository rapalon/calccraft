# Ревью MCP-сервера CalcCraft (`calccraft-mcp`)

Дата: 2026-11-07 · Автор: агент-ревьюер (задача Mission Control `a76fc9e9`) · Отчёт лежит на ветке
`mc/a76fc9e9`, созданной от `main` (`4ea709b`).

## Итог и вердикт

**Вердикт: merge после исправлений.** В текущем виде вливать нельзя: есть два блокера, оба дают
**молча неверный ответ**, а ради правильных ответов сервер и задуман.

Что сделано хорошо:

- Отдельный крейт и бинарник (`crates/mcp`, `calccraft-mcp`). Транспорт только stdio, порт не
  открывается.
- Движки вынесены в `crates/core`. GUI, WASM и Android не тянут `rmcp`, `schemars` и `serde_json`.
- Все лицензии разрешительные (`cargo deny check licenses` → `licenses ok`).
- `cargo fmt`, `clippy` (native и wasm32) и все 315 тестов workspace проходят.
- Ошибки приходят как `isError`. В stdout не пишется ничего, кроме протокола.
- 220 параллельных вызовов отработали без сбоев.
- Новый вычислитель формул (`engine/expression.rs`) совпал с движком GUI во всех сравнениях.

Что сломано:

1. **`programmer`: длинные числа считаются неверно без ошибки.** Число от 39 цифр переполняет
   `u128` и в release-сборке «заворачивается». `99999…9` (44 цифры) превращается в
   `999999999999999999`, а в debug-сборке тот же ввод вызывает панику.
2. **`convert_units`: `GB`, `MB`, `KB`, `TB` понимаются как гига-/мега-/кило-/терабиты.** Сравнение
   сокращений идёт без учёта регистра, и `1 GB → MiB` даёт `119.2093` вместо `953.6743`. При этом
   `GB` стоит примером прямо в описании инструмента.

Ещё четыре важных проблемы:

- в `programmer` вложенность скобок больше 25 даёт неверный результат без ошибки;
- таймаут не прерывает уже запущенное вычисление;
- неизвестные параметры молча игнорируются: `"word"` вместо `"word_size"` даёт QWORD;
- ветка отстала от `main` на 6 коммитов, и при слиянии конфликтует `Cargo.toml`.

Всё это чинится за 1–2 дня: в каждой находке ниже указаны место и способ исправления.

## Что ревьюилось

- **Ветка:** `origin/feature/mcp-server`, **коммит `1fc5e34210b30cde304154cc8b248f33da86cbdb`**
  («Add calccraft-mcp, an MCP server that lets AI agents use the calculators», 2026-10-18).
  Перед ним идёт коммит плана `835723a` («Add a plan for an MCP server…»). База ветки — `1567714`
  (v1.3.0). `main` с тех пор ушёл на 6 коммитов вперёд (`4ea709b`, v1.3.2).
- **Как искал:** выполнил `git fetch origin --prune`, просмотрел `git branch -r`,
  `git branch -r --no-merged origin/main` и `git log --all -i --grep=mcp`. Затем прошёл
  `git grep -i -e mcp -e rmcp` по всем невлитым веткам (`feature/license-check`, `mc/21fb9fbb`,
  `mc/bfb2851a`, `mc/c6afd429`, `mc/f34b5f7d`, `research/mobile-apps`).
- **Результат поиска:** код MCP есть только в `feature/mcp-server`. В `mc/f34b5f7d` есть лишь
  предложение в `docs/next-steps.md`, в остальных ветках нет ничего. Ветки `research/mcp-server`,
  которую упоминает план, на origin нет.
- **Как проверял:** ветку выписал в отдельный worktree (detached, без изменений) и собрал
  release- и debug-сборки на Windows 11 (rustc 1.95+, MSVC). Сервер гонял своим минимальным
  MCP-клиентом на Node.js: `initialize` → `notifications/initialized` → `tools/list` /
  `tools/call` по JSON-RPC через stdio. Спорные случаи сравнил с движком GUI: временный крейт
  подключал `calccraft-core` и вводил те же выражения через `Calculator::paste` и команды
  клавиш.

## 1. Что это такое

| Вопрос | Ответ |
| --- | --- |
| Форма | Отдельный крейт `crates/mcp` (`calccraft-mcp`, бинарник). Это не флаг GUI-приложения. Движки перенесены в `crates/core` (`calccraft-core`), GUI (`calculator`) зависит от него и реэкспортирует модули (`src/lib.rs`). |
| SDK | `rmcp` 3.5.1 (`server`, `macros`, `transport-io`), `schemars` 1.2.3 (через rmcp), `tokio` 1.53 (current-thread + `spawn_blocking`), `serde`/`serde_json`. |
| Транспорт | Только **stdio** (`rmcp::transport::stdio()`). Сеть не слушает, поэтому аутентификация не нужна. |
| Версии протокола | Сервер принимает `2024-11-05`, `2025-03-26`, `2025-06-18` и `2025-11-25`. На запрос `2026-07-28` и на неизвестную версию отвечает `2025-11-25`. |
| Запуск | `cargo build --release -p calccraft-mcp` собирает `target/release/calccraft-mcp(.exe)` размером 4.0 МБ. Запускает его MCP-клиент. Флаги `--version` и `--help` печатают в stdout и сразу выходят, на неизвестный аргумент сервер отвечает кодом 2 и пишет в stderr. |
| Подключение | `crates/mcp/README.md` описывает подключение: `claude mcp add calccraft -- /path/to/calccraft-mcp`, `claude_desktop_config.json` (`mcpServers`), `.vscode/mcp.json`, Cursor, MCP Inspector. |
| Инструменты | Шесть инструментов: `calculate`, `programmer`, `convert_units`, `date_difference`, `date_add` и `analyze_function`. У всех есть `title`, `description`, `inputSchema` и `outputSchema` (схемы генерирует schemars) и аннотации `readOnlyHint: true`, `destructiveHint: false`, `idempotentHint: true`, `openWorldHint: false`. Результат отдаётся как `structuredContent` и тот же JSON текстом. |

Входы и выходы инструментов (по `tools/list`):

| Инструмент | Вход (обязательные **жирным**) | Выход |
| --- | --- | --- |
| `calculate` | **`expression`**; `angle`: degrees (по умолчанию) / radians / gradians; `mode`: scientific (по умолчанию) / standard | `result` (32 значащие цифры, в standard 16); `fraction` (`p/q`, если результат точный и не целый) |
| `programmer` | **`expression`**; `radix`: hex / dec (по умолчанию) / oct / bin; `word_size`: qword (по умолчанию) / dword / word / byte | `hex`, `dec` (со знаком), `oct`, `bin`, `expression_line` |
| `convert_units` | **`category`**, **`value`** (строка), **`from`**, **`to`** | `value`, `from`, `to` (полные имена единиц), `about_equal_to[]` |
| `date_difference` | **`from`**, **`to`** (YYYY-MM-DD) | `years`, `months`, `weeks`, `days`, `total_days`, `description` |
| `date_add` | **`date`**; `years`, `months`, `days` (u32); `subtract` | `date` (ISO), `long_date` |
| `analyze_function` | **`equation`**; `angle` | `domain`, `range`, `x_intercepts`, `y_intercept`, `minima[]`, `maxima[]`, `inflection_points[]`, `vertical/horizontal/oblique_asymptotes[]`, `parity`, `period`, `monotonicity[]`, `too_complex[]` |

## 2. Сборка и влияние на проект

| Проверка (на `1fc5e34`) | Результат |
| --- | --- |
| `cargo fmt --all --check` | ✅ без замечаний |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo clippy --target wasm32-unknown-unknown --workspace --exclude calccraft-mcp -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 315 тестов: core 218 (+6 ignored), app 85 (+2 ignored), MCP unit 9, MCP stdio 3. На `main` (`cargo test`, v1.3.2): 296 passed, 8 ignored. В базе ветки (v1.3.0) было 295, план называет то же число; 295 + 8 новых тестов `expression.rs` = 303 = 218 + 85, так что при переносе в `core` ни один тест не потерян. |
| `cargo deny check licenses` (конфиг из `feature/license-check`, все 6 целевых платформ, весь workspace) | ✅ `licenses ok` |
| Есть ли `rmcp` в дереве GUI (`cargo tree -p calculator --target all -i rmcp`) | ✅ нет (`did not match any packages`). `tokio` в дереве GUI был и на `main` (через `wgpu`/`wasm-bindgen-futures`), это не новое. |
| Размер release `calculator.exe` | `main` (v1.3.2): 22 061 568 байт; ветка: 22 098 432 байт (**+36 КБ, +0.17 %**), хотя в ветке нет 6 новых коммитов `main`. Скорее всего, это `engine/expression.rs` и `pub` API ядра. Влияние пренебрежимое. |
| CI (`ci.yml`) | Переведён на `--workspace`: fmt, clippy и тесты MCP (включая stdio-тест) идут на Windows, Linux и macOS. Wasm-clippy исключает `calccraft-mcp`. Android-clippy проверяет корневой пакет, а значит и `core`. |
| Release (`release.yml`) | Тесты переведены на `--workspace`, но **бинарник MCP не собирается и не публикуется** (`cargo build --release --target …` собирает только корневой пакет). README честно пишет, что это фаза 4. |
| Feature-gate | Не нужен: это отдельный member workspace. Корневые команды без `--workspace` собирают только приложение. |

Лицензии зависимостей сервера (`cargo deny list` для `crates/mcp`): MIT и/или Apache-2.0 у всех,
плюс `unicode-ident` (Unicode-3.0) и `memchr` (Unlicense/MIT). Копилефта и неясных лицензий нет.
Новые для проекта крейты: `rmcp`, `rmcp-macros`, `schemars(+derive)`, `serde_json`,
`tokio-util`, `tokio-stream`, `process-wrap` (dev, для клиента в тесте), `pastey`, `darling`,
`ref-cast`, `dyn-clone` и другие. Все они MIT/Apache.

## 3. Находки

### Blocker

#### B1. `programmer`: число от 39 цифр молча заменяется другим (переполнение `u128`)

`crates/mcp/src/tools.rs:349`:
`value = value.saturating_mul(u128::from(base)) + u128::from(digit);`.

После насыщения `+ digit` переполняется:

- в debug-сборке это паника `attempt to add with overflow`, и клиент получает
  `"The calculation failed"`;
- в release-сборке (`overflow-checks` выключены) значение «заворачивается» в маленькое число и
  проходит проверку диапазона. После этого движок, как клавиатура Windows, отбрасывает лишние
  цифры, и **ответ получается без ошибки, но про другое число**.

```
programmer {"expression":"99999999999999999999999999999999999999999999"}   (44 цифры, release)
  -> {"dec":"999999999999999999","hex":"DE0B6B3A763FFFF","expression_line":"999999999999999999 =", …}
programmer {"expression":"340282366920938463463374607431768211456"}         (2^128, release)
  -> {"dec":"3402823669209384634","hex":"2F394219248446BA", …}
то же в debug-сборке -> isError: The calculation failed
  stderr: thread 'tokio-rt-worker' panicked at crates\mcp\src\tools.rs:349:17: attempt to add with overflow
```

С 20 цифрами (`18446744073709551616`) проверка ещё срабатывает: «doesn't fit». Ломается только
от 39 цифр.

**Исправление:** `checked_mul`/`checked_add` с ошибкой «doesn't fit». Ещё лучше сначала
ограничить число цифр размером слова (64 бит → 20 dec / 16 hex / 22 oct / 64 bin) и добавить
тест на 40-значное число в debug и release.

#### B2. `convert_units`: `GB`/`MB`/`KB`/`TB` — это биты, а не байты

`find` в `tools.rs:439–461` сравнивает имя, ключ и сокращение через `to_lowercase()` и берёт
первое совпадение. В категории Data «Gigabits» (`Gb`) идёт раньше «Gigabytes» (`GB`), поэтому
`GB` → Gigabits, `MB` → Megabits, `KB` → Kilobits, `TB` → Terabits.

```
convert_units {"category":"data","value":"1","from":"GB","to":"MiB"}
  -> {"from":"Gigabits","to":"Mebibytes","value":"119.2093", …}      (должно быть 953.6743)
convert_units {"category":"data","value":"1","from":"gigabytes","to":"MiB"}
  -> {"from":"Gigabytes","to":"Mebibytes","value":"953.6743"}        (верно только с полным именем)
convert_units {"category":"data","value":"1","from":"KB","to":"B"}
  -> {"from":"Kilobits","to":"Bits","value":"1000"}                  (должно быть 1000 Bytes)
```

Описание инструмента прямо приводит пример `(km, mi, °C, GB)`. Юнит-тест
`convert("data", "1", "GB", "MB") == "1000"` проходит случайно: биты→биты дают тот же коэффициент.
Агент увидит `from: "Gigabits"`, только если специально проверит поле.

**Исправление:** сначала искать точное совпадение сокращения с учётом регистра, а без учёта
регистра — только если оно однозначно. При неоднозначности возвращать ошибку со списком вариантов.
Добавить тесты для `GB/Gb`, `MB/Mb`, `KB/Kb`.

### Important

#### I1. `programmer`: больше 25 уровней скобок — молча неверный результат

Движок, как и Windows, держит не больше 25 открытых скобок, а лишние `OpenParen` игнорирует. В
GUI кнопку просто нельзя нажать. MCP-слой (`programmer`, цикл по токенам) этот предел не
проверяет:

```
programmer  "(((…(1+1)*2+1)*2 … +1)*2"  — 40 уровней (ожидается 3298534883326)
  -> {"dec":"100663324", "expression_line":"(((((((((((((((((((((((((1 + 1) × 2 … × 2 + 1 × 2 + 1 × 2 =", …}
то же, 24 уровня -> 50331646 (верно)
то же через клавиши движка GUI (30 уровней) -> 100,663,304 (движок ведёт себя так же; проблема в отсутствии проверки в MCP)
```

У `calculate` своя рекурсия с пределом 100: 30 уровней дают `3221225470`, верно.

**Исправление:** считать глубину в `programmer` и при глубине больше 25 возвращать понятную ошибку.

#### I2. Таймаут 10 с не прерывает вычисление, параллелизм не ограничен

`server.rs:28–39`: `tokio::time::timeout(BUDGET, spawn_blocking(work))`. По таймауту отбрасывается
только `JoinHandle`, а поток `spawn_blocking` дорабатывает до конца: у tokio его не отменить.
Ограничения на число одновременных тяжёлых вызовов тоже нет (по умолчанию пул до 512 потоков).

Медленный ввод я не нашёл: самые тяжёлые случаи шли меньше 60 мс
(`3248.5!`, `1000.5!`×60, `(123456789/987654321)^9999`, `sin(1e-9999)`,
`asin(1e-9999)`, `sin(1000x)+cos(999x)` в анализе, 512-значные числа). Поэтому реальный риск
сейчас невелик. Но гарантия «no input should hang the server» держится на лимитах движков, а не
на таймауте. Кроме того, план обещал 2 с и `catch_unwind`, а в коде 10 с, и паника превращается
в безликое `"The calculation failed"`.

**Исправление:** добавить семафор на 2–4 одновременных вычисления. Для долгих мест (анализ,
`power`, `factorial`) — флаг отмены или бюджет шагов. Записывать в stderr, что вычисление
упёрлось в таймаут или упало с паникой.

#### I3. Неизвестные и опечатанные параметры молча игнорируются

В схемах нет `additionalProperties: false`, а у структур — `#[serde(deny_unknown_fields)]`:

```
programmer {"expression":"-1","word":"byte"}            (имя параметра из docs/mcp-server-plan.md)
  -> {"hex":"FFFFFFFFFFFFFFFF", …}                        (молча QWORD; с "word_size":"byte" -> "FF")
calculate {"expression":"sin(90)","angles":"radians"}   -> {"result":"1"}   (молча градусы)
calculate {"expression":"1+1","extra":true}             -> {"result":"2"}
```

Ограничения в схемах тоже не объявлены: нет `maxLength: 512`, хотя план их описывал, и нет
`maximum: 999` у `years`/`months`/`days`. Проверяются они только в коде.

**Исправление:** `deny_unknown_fields`, а лимиты передать в схему через `#[schemars(...)]`.

#### I4. Ветка отстала от `main`, конфликт в `Cargo.toml`

База ветки — `1567714` (v1.3.0). `git merge-tree origin/main origin/feature/mcp-server` даёт
`CONFLICT (content): Merge conflict in Cargo.toml`: версия переехала в `[workspace.package]`, а
в `main` её подняли до 1.3.2.

Конфликт тривиальный, но после rebase нужно перезапустить все проверки. Шесть коммитов `main`
(pinch-zoom, haptics, правки Android) трогают `src/app*` и `android/`, а не перенесённые модули,
так что автослияние переименований прошло. Ещё стоит решить, будет ли у MCP-сервера та же версия,
что у приложения (сейчас `--version` выдаёт `1.3.0`).

#### I5. `convert_units`: слабое сопоставление имён единиц

Описание обещает «Units by English name or abbreviation», но работают только:

- полное имя (`kilometers`);
- имя без последней `s`;
- ключ (`foot`, `inch`);
- сокращение из таблицы Windows.

```
volume  "gallon" -> isError: Unknown unit 'gallon' … Gallons (US) (gal (US)) …   ("gal (US)" работает)
volume  "cup"    -> isError: Unknown unit 'cup' …
power   "hp"     -> isError: Unknown unit 'hp' … Horsepower (US) (hp (US)) …
```

Ошибка хорошо перечисляет допустимые имена, так что агент поправится со второй попытки, но это
лишние вызовы. Неоднозначность из B2 растёт из той же функции.

**Исправление:** таблица синонимов (`gallon` → US, `hp`, `cup`, `GB`/`Gb` …) с тестами.

### Minor

- **M1. `-128` в BYTE не принимается.** `programmer {"expression":"-128","word_size":"byte"}` →
  `128 doesn't fit into a Byte`. В Windows и в GUI это допустимо (AGENTS.md: «A typed negative
  number that wouldn't fit as a positive one keeps its sign (`-128` in a byte)»). `type_number`
  дублирует проверку движка (`digit_fits`) и не знает о знаке. То же для `-32768` в WORD и так
  далее.
- **M2. Сообщения Standard-режима вводят в заблуждение.** `"12345678901234567 + 1"` (17 цифр при
  пределе 16) получает ответ «the Standard mode takes only numbers and + - * /». Сама строка
  допустима, причина в длине операнда. Кроме того, `"2 + 3 * 4 = ="` в standard даёт `80`:
  `trim_end_matches('=')` срезает только последний `=`, а оставшийся `=` повторяет операцию.
- **M3. Нет ответа на не-JSON.** Строка `this is not json` остаётся без ответа (по JSON-RPC
  ожидается `-32700 Parse error`). Сервер при этом продолжает работать: следующий `calculate 1+1`
  отвечает `2`. Скорее всего, это поведение rmcp.
- **M4. Расхождения документации с кодом.**
  - `docs/mcp-server-plan.md` обещает бюджет 2 с и `catch_unwind`, в коде 10 с и `spawn_blocking`.
  - План обещает поля `exact` и `expression_line` у `calculate`, в коде есть только `fraction`.
  - План называет параметр `word`, в коде он `word_size` (см. I3).
  - План обещает snapshot-тест `tools/list` и fuzz-цель для парсера («Done when… a fuzz target
    that never panics»). Их нет.
  - Шапка плана ссылается на ветку `research/mcp-server`, которой нет на origin.
  - План утверждает, что rmcp 3.x поддерживает MCP 2026-07-28, но сервер на эту версию отвечает
    `2025-11-25`.

  `crates/mcp/README.md` пишет «offline, without running any code». Явной фразы «не собирает и не
  отправляет никаких данных, не читает файлы и переменные окружения» нет. По коду это так (см.
  раздел 4), и такую фразу стоит добавить.
- **M5. `%` — это остаток, и `x mod 0 = x`.** Так ведёт себя Windows, в описании это указано. Но
  агент, написавший `200 % 15`, получит `5` без предупреждения, а `10 % 0` даст `10`. Стоит прямо
  написать в описании «% is NOT percent» и упомянуть `x mod 0 = x`.
- **M6. Унаследованные странности анализа функций (из `core`, в GUI так же).**
  - `ln(x)/sqrt(x)`: горизонтальная асимптота `y = −0.00000150496`, должна быть `y = 0`.
  - `x^x`: `domain {x ∈ ℝ}` и `Constant` на `(−∞, 0.367879)`.
  - `x^x^…^x` (41): `domain {x ≤ 0}`.

  Это не вина MCP-слоя, но через MCP агенту уходят «авторитетные» ответы. Можно добавить в
  описание оговорку или завести отдельные задачи на `analysis.rs`.
- **M7. Тесты MCP-слоя покрывают только «счастливые пути».** Нет тестов на B1, B2, I1 и I3, нет
  snapshot-теста схем и fuzz-теста. Хватило бы одного теста с 40-значным числом в debug-сборке,
  чтобы поймать B1.
- **M8. Мелочи кода.**
  - `CATEGORIES[category as usize]` держится на порядке вариантов `enum Category`; надёжнее
    `match`.
  - `english.rs` и тест в `converter.rs` делают `include_str!("../../../locales/en-US.txt")`, так
    что крейты нельзя опубликовать на crates.io (план упоминает это как опцию).
  - Собственный токенизатор `programmer` в MCP-крейте дублирует правила движка (лимиты цифр, знак).
    Логичнее держать его рядом с `engine/expression.rs` в `core`.

## 4. Безопасность и устойчивость

- **Файлы, сеть, окружение, процессы.** В `crates/mcp/src` есть только `std::env::args()` (флаги)
  и `println!` для `--version`/`--help` до старта протокола. `std::fs`, `std::env::var`,
  `std::process`, `std::net` встречаются только в тестовом коде ядра (`monkey.rs`,
  `ms_oracle.rs`, под `#[cfg(test)]`) и в тесте `tests/stdio.rs`. Сетевых или HTTP-фич rmcp нет
  (`transport-io` — это stdio). ✅
- **stdout.** За всё время в stdout не было ни одной строки, кроме JSON-RPC (клиент проверял каждую
  строку). Паники и ошибки идут в stderr. ✅
- **Ошибки.** Ошибки вычислений и разбора приходят как `isError: true` с понятным текстом.
  Неверные типы и отсутствующие поля дают `isError` «failed to deserialize parameters: …».
  Неизвестный инструмент даёт JSON-RPC `-32602 tool not found`. Паника в инструменте стала
  `isError` «The calculation failed», и сервер не упал. ✅
- **Размер и глубина.** Формула ограничена 512 символами (`513` → ошибка), уравнение — 2048,
  рекурсия — 100 уровнями. Проверено: 99 скобок, 99 минусов и 49 вложенных `sqrt` проходят;
  `sin(x)+…`×200 → «Invalid expression». ✅
- **Огромные числа.** `9^9^9`, `7^(2^20)` и `3^3^3^3` дают `Invalid input`, так же как движок GUI
  через клавиши. `2^99999`, `1e10000`, `exp(99999)`, `cosh(10^5)`, `(2^9999)!` и `3249!` дают
  `Overflow`. Всё отвечает за миллисекунды. ✅
- **Не-ASCII.** `√16 + ∛27 + 3² × π ÷ π` → `16`. Полноширинные цифры, эмодзи и греческие буквы
  дают «Unknown character 'x' (at character N)» без паники. ✅
- **Параллельность.** 220 одновременных запросов (200 `calculate` и 20 `analyze_function`) все
  получили верный ответ, ошибок и таймаутов нет. ✅ (См. I2 о неотменяемых потоках.)

## 5. Таблица проверенных случаев

Все случаи прогнаны через протокол (stdio, JSON-RPC) на release-сборке `1fc5e34`, если не
сказано иное. «✅» — совпадает с ожидаемым и с поведением Windows/GUI, «❌» — ошибка, «⚠️» —
спорно.

| # | Инструмент | Вход | Фактический результат | Оценка |
| --- | --- | --- | --- | --- |
| 1 | calculate | `0.1+0.2-0.3` | `0` | ✅ |
| 2 | calculate | `1/3*3` | `1` (без fraction) | ✅ |
| 3 | calculate | `1/3` | `0.33333333333333333333333333333333`, fraction `1/3` | ✅ 32 цифры |
| 4 | calculate | `2/3` | `0.66666666666666666666666666666667`, `2/3` | ✅ |
| 5 | calculate | `2+3*4` / `(2+3)*4` | `14` / `20` | ✅ |
| 6 | calculate | `2^3^2` / `-2^2` / `2^-1` | `512` / `-4` / `0.5` (`1/2`) | ✅ |
| 7 | calculate | `sqrt(2)` / `2^0.5` | `1.4142135623730950488016887242097` (оба; совпадает с клавишами GUI) | ✅ |
| 8 | calculate | `sqrt(2)^2` / `root(27,3)` / `(-8)^(1/3)` / `cbrt(-8)` | `2` / `3` / `-2` / `-2` | ✅ |
| 9 | calculate | `root(-16, 4)` | Invalid input | ✅ |
| 10 | calculate | `5!` / `25!` | `120` / `15511210043330985984000000` | ✅ |
| 11 | calculate | `0.5!` | `0.88622692545275801364908374167057` (= √π/2; клавиши GUI дают то же) | ✅ |
| 12 | calculate | `(-0.5)!` / `(-1)!` | `1.7724538509055160272981674833411` (√π) / Invalid input | ✅ |
| 13 | calculate | `3249!` / `3250!` / `3248.5!` | Overflow / Overflow / `1.1249282436388776609851889735984e+9999` | ✅ |
| 14 | calculate | `sin(30)` (deg) / `sin(30)` (rad) / `sin(pi/6)` (rad) | `0.5` / `-0.98803162409286178998774890729446` / `0.5` | ✅ |
| 15 | calculate | `cos(100)` grad / `tan(45)` / `tan(90)` | `0` / `1` / Invalid input | ✅ |
| 16 | calculate | `asin(2)` / `atanh(1)` / `ln(0)` / `sqrt(-1)` | Invalid input / Cannot divide by zero / Invalid input / Invalid input | ✅ |
| 17 | calculate | `pi` / `e` / `ln(e)` / `e^2` | `3.1415926535897932384626433832795` / `2.7182818284590452353602874713527` / `1` / `7.389056098930650227230427460575` | ✅ |
| 18 | calculate | `log(1000)` / `log(2, 1024)` / `log(0, 5)` | `3` / `10` / Invalid input | ✅ |
| 19 | calculate | `123456789012345678901234567890 * 987654321` | `1.2193263112482853211248285321113e+38` (= клавиши GUI) | ✅ |
| 20 | calculate | `2^100` / `1e-20` | `1267650600228229401496703205376` / `0.00000000000000000001` (`1/100000000000000000000`) | ✅ |
| 21 | calculate | `10 mod 3` / `-10 mod 3` / `10 % 0` | `1` / `2` / `10` | ✅ как Windows (⚠️ M5) |
| 22 | calculate | `1/0` / `0/0` | Cannot divide by zero / Result is undefined | ✅ |
| 23 | calculate | `1e10000` / `10^9999` / `2^99999` | Overflow / `1.e+9999` / Overflow | ✅ |
| 24 | calculate | `9^9^9`, `7^(2^20)` | Invalid input (так же у клавиш GUI) | ✅ совпадает с движком |
| 25 | calculate | `0^0` / `0^-1` | `1` / Invalid input | ✅ |
| 26 | calculate | `2pi` / `2^3!` / `-3!` | `6.283185307179586476925286766559` / `64` / `-6` | ✅ |
| 27 | calculate | `2 3`, `1,000 + 1`, `2 +`, `x + 1`, `sin 30`, `` | понятные синтаксические ошибки с позицией | ✅ |
| 28 | calculate | `√16 + ∛27 + 3² × π ÷ π` | `16` | ✅ |
| 29 | calculate | `２＋２`, `1 + 1 😀`, `ασ` | Unknown character … (at character N) | ✅ |
| 30 | calculate | `"angle":"turns"` / `expression: 42` / `{}` | isError: failed to deserialize parameters … | ✅ |
| 31 | calculate | `"extra": true` / `"angles":"radians"` | молча игнорируется | ❌ I3 |
| 32 | calculate | 512-значное число / 513 символов | `1.1111111111111111111111111111111e+511` / «longer than 512» | ✅ |
| 33 | calculate | 99 скобок / 99 минусов / 49 вложенных sqrt / 30 уровней `(…+1)*2` | `1` / `-1` / `1.0000000000000012312767348986599` / `3221225470` | ✅ |
| 34 | calculate standard | `2 + 3 * 4` / `1/3` / `1/7` / `1/0` | `20` / `0.3333333333333333` / `0.1428571428571429` / Cannot divide by zero | ✅ |
| 35 | calculate standard | `12345678901234567 + 1` | «takes only numbers and + - * /» | ⚠️ M2 |
| 36 | calculate standard | `2 + 3 * 4 = =` | `80` | ⚠️ M2 |
| 37 | programmer | `FF AND 3C` hex byte | `3C` / `60` / `74` / `00111100` | ✅ |
| 38 | programmer | `-1` dec word | `FFFF`, dec `-1` | ✅ |
| 39 | programmer | `127 + 1` byte | `80` / `-128` | ✅ |
| 40 | programmer | `-128` byte | «128 doesn't fit into a Byte» | ❌ M1 |
| 41 | programmer | `1 << 4` / `1 LSH 64` / `-16 RSH 2` | `16` / Result not defined / `-4` | ✅ |
| 42 | programmer | `ROL(80)` hex byte / `ROR(1)` hex dword / `NOT 0` hex dword | `1` / `80000000` / `FFFFFFFF` | ✅ |
| 43 | programmer | `1010 XOR 0110` bin / `777 + 1` oct | `1100` / `1000` | ✅ |
| 44 | programmer | `-7 / 2` / `-7 % 3` / `5 % 0` / `5 / 0` | `-3` / `-1` / Result is undefined / Cannot divide by zero | ✅ |
| 45 | programmer | `4 OR 2 + 2` | `4` | ✅ |
| 46 | programmer | `FFFFFFFFFFFFFFFF + 1` hex / `9223372036854775807 + 1` | `0` / `-9223372036854775808` | ✅ |
| 47 | programmer | 44 девятки (dec) | release: `999999999999999999`; debug: The calculation failed (паника) | ❌ B1 |
| 48 | programmer | `340282366920938463463374607431768211456` | release: `3402823669209384634` | ❌ B1 |
| 49 | programmer | 40 уровней `(…+1)*2` | `100663324` вместо `3298534883326` | ❌ I1 |
| 50 | programmer | 201 скобка вокруг `1` / 300 минусов перед `1` / `1+1+…`×128 | `1` / `1` / `128` | ✅ (результаты случайно верны) |
| 51 | programmer | `0x10` / `1G` hex | «'x' isn't a DEC digit» / «'G' isn't a HEX digit» | ✅ |
| 52 | programmer | `"word":"byte"` вместо `word_size` | молча QWORD (`FFFFFFFFFFFFFFFF`) | ❌ I3 |
| 53 | convert_units | length 5 km → mi | `3.106856` + about_equal_to | ✅ |
| 54 | convert_units | length 1 inch → cm / 1 foot → m / 1 inches → mm | `2.54` / `0.3048` / `25.4` | ✅ |
| 55 | convert_units | temperature 100 °C → °F / -40 celsius → fahrenheit | `212` / `-40` | ✅ |
| 56 | convert_units | weight 1 kg → lb / 1 t → kg | `2.204623` / `1000` | ✅ |
| 57 | convert_units | data 1 GB → MB / 1 GB → MiB / 1 KB → B / 1 TB → GB | Gigabits→Megabits `1000` / Gigabits→Mebibytes `119.2093` / Kilobits→Bits / Terabits→Gigabits | ❌ B2 |
| 58 | convert_units | data 1 GiB → MiB / 1 gigabytes → MiB | `1024` / `953.6743` | ✅ |
| 59 | convert_units | speed 100 km/h → mph / 1 knot → km/h | `62.14268` / `1.85184` | ✅ |
| 60 | convert_units | pressure 1 atm → Pa / 1 bar → psi | `101325` / `14.50377` | ✅ |
| 61 | convert_units | angle 180 degrees → radians | `3.141593` | ✅ |
| 62 | convert_units | time 1 day → seconds / 1 ms → s | `86400` / `0.001` | ✅ |
| 63 | convert_units | energy 1 kWh → J / 1 calorie → joule | `3600000` / `4.184` (Thermal calories) | ✅ |
| 64 | convert_units | area 1 acre → m² | `4046.856` | ✅ |
| 65 | convert_units | volume 1 gal (US) → L / 1 gallon → liter / 1 cup → mL | `3.785412` / Unknown unit / Unknown unit | ⚠️ I5 |
| 66 | convert_units | power 1 hp → W | Unknown unit 'hp' | ⚠️ I5 |
| 67 | convert_units | length -1 m / `1e3` / 19 цифр / parsec / currency | понятные ошибки | ✅ |
| 68 | date_difference | 2026-01-01 ↔ 2026-10-18 (в обе стороны) | 0 y 9 m 2 w 3 d, 290 дней, «9 months, 2 weeks, 3 days» | ✅ |
| 69 | date_difference | 2024-02-29 → 2025-02-28 | «1 year», 365 | ✅ |
| 70 | date_difference | одинаковые даты / +3 дня | «Same dates» / «3 days» | ✅ |
| 71 | date_difference | 1601-01-01 → 9999-12-31 | 8398 y 11 m 4 w 2 d, 3067670 | ✅ |
| 72 | date_difference | `2026-02-30` / `07.11.2026` | «isn't a date: write it as YYYY-MM-DD» | ✅ |
| 73 | date_add | 2026-01-31 + 1 month | `2026-02-28`, «Saturday, February 28, 2026» | ✅ |
| 74 | date_add | 2024-02-29 + 1 year | `2025-02-28` | ✅ |
| 75 | date_add | 2026-03-31 − 1 month 1 day | `2026-02-28` | ✅ (как в AGENTS.md) |
| 76 | date_add | 9999-12-31 + 1 day / days 1000 / days −1 | Date out of Bound / «0 to 999» / failed to deserialize | ✅ |
| 77 | analyze_function | `y = 1/x` | domain `{x ≠ 0}`, range `{y ∈ ℝ∖{0}}`, асимптоты `x = 0`, `y = 0`, odd | ✅ |
| 78 | analyze_function | `x^2 - 4x + 3` | range `{y ∈ [−1, ∞)}`, min `(2, −1)`, нули `1, 3` | ✅ |
| 79 | analyze_function | `sin(x)` rad / deg | период `2π` / `360`, семейства экстремумов | ✅ |
| 80 | analyze_function | `tan(x)` rad | domain `{x ≠ πn₁ + π/2, ∀n₁ ∈ ℤ}`, период `π` | ✅ |
| 81 | analyze_function | `sqrt(25 - x^2)` | domain `{x ∈ [−5, 5]}`, even | ✅ |
| 82 | analyze_function | `1/(x^2+1)` / `(x^5 - 3x^2 + 7)/(x^2-5)` | точки перегиба `(±√3/3, 3/4)` / domain `{x ≠ −√5, x ≠ √5}` | ✅ |
| 83 | analyze_function | `ln(x)/sqrt(x)` | горизонтальная асимптота `y = −0.00000150496` | ⚠️ M6 |
| 84 | analyze_function | `x^x` | domain `{x ∈ ℝ}`, Constant на `(−∞, 0.367879)` | ⚠️ M6 |
| 85 | analyze_function | `x)` / `x = y^2` / `y = mx + b` / `y > x` | тексты ошибок Windows | ✅ |
| 86 | протокол | 220 параллельных вызовов | все ответили верно | ✅ |
| 87 | протокол | строка не-JSON | ответа нет, сервер жив | ⚠️ M3 |
| 88 | протокол | неизвестный инструмент | JSON-RPC `-32602 tool not found` | ✅ |

## 6. Рекомендуемые шаги

1. Исправить B1 и B2 (вместе с тестами) — после этого блокеров нет.
2. Исправить I1 (предел 25 скобок) и I3 (`deny_unknown_fields`, лимиты в схеме), добавить
   синонимы единиц (I5) и `-128` в BYTE (M1).
3. Сделать rebase на `main` (v1.3.2), разрешить конфликт `Cargo.toml` и решить вопрос версии
   сервера (I4). Затем прогнать fmt, clippy (native, wasm32, Android), `cargo test --workspace`
   и сборку APK.
4. Добавить семафор и логирование таймаутов и паник (I2). Планово — отмену долгих вычислений.
5. Тесты: snapshot `tools/list`, «злые» входы для каждого инструмента, прогон stdio-теста и на
   release-сборке (B1 проявляется только там), fuzz-цель для `expression.rs` и токенизатора
   `programmer`.
6. Документация: сверить `docs/mcp-server-plan.md` с кодом (M4). В `crates/mcp/README.md`
   добавить явную фразу о том, что сервер работает локально и не собирает данных, а также
   предупреждения про `%`/`mod` (M5) и про числовой характер анализа (M6).
7. Фаза 4 (упаковка): сборка `calccraft-mcp` в `release.yml` для четырёх десктопных целей и
   `.mcpb`. Отдельно — `deny.toml` из `feature/license-check` в CI, чтобы новые зависимости
   rmcp проверялись автоматически.

## Как повторить проверку

```bash
git fetch origin
git worktree add --detach ../mcp-review 1fc5e34210b30cde304154cc8b248f33da86cbdb
cd ../mcp-review
cargo build --release -p calccraft-mcp
cargo test --workspace
npx @modelcontextprotocol/inspector target/release/calccraft-mcp
#   B1: programmer {"expression": "99999999999999999999999999999999999999999999"}
#   B2: convert_units {"category": "data", "value": "1", "from": "GB", "to": "MiB"}
#   I1: programmer {"expression": "((((((((((((((((((((((((((((1+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2+1)*2"}
```
