# Kозацький бізнес на Solana

Повний Anchor workspace для тестового завдання WhiteBIT / НаУКМА.

## Що реалізовано

Проєкт розбитий на 6 окремих програм, як того вимагає умова:

- `resource_manager` — конфігурація гри, створення ресурсних Token-2022 mint-ів, mint для Search, burn для Crafting
- `item_nft` — створення унікальних NFT-предметів через Metaplex
- `crafting` — перевірка рецептів, burn ресурсів і mint предмета
- `search` — пошук ресурсів раз на 60 секунд через Player PDA
- `marketplace` — викуп предмета у гравця, burn NFT, mint MagicToken
- `magic_token` — окремий Token-2022 mint для винагороди в Marketplace

## Архітектурне рішення

Умова репозиторію має неоднозначність у Marketplace:
- з одного боку написано, що гравці продають предмети за MagicToken
- з іншого боку покупець після купівлі начебто або отримує NFT, або NFT спалюється

Щоб зберегти узгоджену логіку з фразою, що `MagicToken` можна отримати лише через продаж предметів, тут обрано модель:
- гравець здає предмет у Marketplace
- NFT спалюється
- гравець отримує MagicToken
- окремого покупця в цій реалізації немає

Це найпослідовніше трактування специфікації для on-chain гри.

## Важливі технічні рішення

### 1. Розподіл доступу через окремі PDA-авторитети
Кожна міжпрограмна взаємодія виконується через окрему PDA-роль:

- `search-authority`
- `crafting-authority`
- `marketplace-authority`
- `resource-manager-authority`
- `item-authority`
- `magic-mint-authority`

Тобто Search, Crafting і Marketplace не отримують прямий admin-контроль над чужими mint-ами. Вони підписують лише власними PDA, а цільова програма окремо перевіряє, чи саме цей caller-authority їй дозволений.

### 2. Resource mint-и — Token-2022
Базові ресурси створюються як Token-2022 mint-и з `MetadataPointer`. У цій версії метадані ініціалізуються на самому mint-акаунті.

### 3. NFT-предмети — Metaplex
Предмети карбуються в `item_nft` як окремі mint-и з:
- metadata account
- master edition
- supply = 1

### 4. Search cooldown
У `search` для кожного гравця ведеться `Player` PDA:
- owner
- last_search_timestamp
- bump

Cooldown жорстко зафіксований як 60 секунд.

## Структура

```text
programs/
  resource_manager/
  item_nft/
  crafting/
  search/
  marketplace/
  magic_token/
tests/
  kazak-business.ts
scripts/
  bootstrap.ts
```

## Program IDs

Ці ID уже прописані в `Anchor.toml` і в `declare_id!`:

- resource_manager: `6vR1JhhHdV84XfT6VRA68C6NNVD3JEYqPeoWb85QMkkC`
- item_nft: `9hQ6kKu3bavUxBJj3iebsW4WQF8FL9PMC8Yyyb7kWLEG`
- crafting: `38h5QbRE5EsjNn6ER9wLQNYz69GAMh56gepGY3i6xeGF`
- search: `9bfA4LZHE61W8TjHnciZkz2CR6twi6gEQxV8bWYjs4p3`
- marketplace: `cQWaqkhv47Tp92LBiDkFpFkaM6qvU8V3uj7vN6apCU9`
- magic_token: `BKpyzheujyFvSmRUyZjDqQMcPj553BMHqindrww6qyJ3`

> Якщо ви хочете реально деплоїти це у власне середовище, після `anchor keys list` / `anchor keys sync` треба оновити ці значення консистентно в усіх програмах та в `Anchor.toml`.

## Інструкції запуску

### 1. Встановити залежності
```bash
yarn install
```

### 2. Зібрати проєкт
```bash
anchor build
```

### 3. Запустити локальні тести
```bash
anchor test
```

### 4. Bootstrap середовища
```bash
yarn bootstrap:localnet
```

### 5. Devnet
```bash
solana config set --url devnet
anchor build
anchor deploy
yarn bootstrap:devnet
```

## Що покривають тести

Файл `tests/kazak-business.ts` задуманий як інтеграційний сценарій:

1. Ініціалізація всіх config PDA
2. Створення 6 ресурсних mint-ів
3. Створення MagicToken mint
4. Ініціалізація Player PDA
5. Search з генерацією 3 ресурсів
6. Перевірка cooldown
7. Crafting предмета
8. Redeem у Marketplace
9. Перевірка, що NFT спалений, а MagicToken нарахований

## Що потребує фінальної валідації перед здачею

Через обмеження цього середовища цей workspace підготовлено як повний кодовий пакет, але його не було скомпільовано тут локально через відсутність встановлених Rust / Solana CLI / Anchor CLI. Перед реальним PR обов’язково зробіть:

1. `anchor build`
2. виправлення можливих дрібних несумісностей версій crate-ів
3. `anchor test`
4. `anchor deploy --provider.cluster devnet`
5. оновлення README фактичними devnet program id і tx examples

## Що я рекомендую доробити перед PR

Щоб максимально наблизити рішення до production-grade рівня:

- додати окремі unit tests для кожної програми
- посилити захист від прямого burn користувачем для NFT та ресурсів
- додати окремі admin-інструкції для зміни цін на предмети
- додати event-логування для Search і Crafting
- додати scripts для повного devnet smoke test

