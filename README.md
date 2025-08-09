# scraping-eprijava
Prikupljanje finansijskih izvjestaja sa sajta eprijava.tax.gov.me

---

## PowerShell implementacija

### Pokretanje

U fajlu **DownloadFinansijskihIzvjestaja.ps1** u vrhu postoji predefinisana lista kompanija čiji finansijski izvještaji će biti preuzeti sa sajta ePrijava.tax.gov.me. 

Skripta se pokreće putem batch fajla **Start.bat**.

### Output

Ispis na ekranu ce biti nalik sljedecem:

```
Prikupljanje podataka za: Codeus (03091627)
Pronadjen: 03091627 - D.O.O. "CODEUS"  ZA PROJEKTOVANJE, RAZVOJ I PRODAJU SOFTVERA - PODGORICA

Download detalja pravnog lica

Pretraga liste finansijskih izvjestaja
Pronadjeno 5 finansijskih izvjestaja

Download finansijskih izvjestaja...
Download izvjestaja br. 55136/2020 za godinu 2020
Download izvjestaja br. 56165/2019 za godinu 2019
Download izvjestaja br. 87751/2018 za godinu 2018
Download izvjestaja br. 60360/2017 za godinu 2017
Download izvjestaja br. 101685/2016 za godinu 2016
```

## Rezultat

Za svako pravno lice definisano listom u vrhu, program će prikupiti sve finansijske izvještaje i smjestiti u pod-folder u formatu: `NAZIV-PRAVNOG-LICA\PIB-GODINA.htm`. Pored finansijskog izvještaja, program će preuzeti i detalje i smjestiti u pod-folder u formatu: `NAZIV-PRAVNOG-LICA\PIB.htm`.

Rezultati se, takođe, "parsiraju" i smještaju u CSV fajl pod nazivom **Results.csv**. Primjer fajla slijedi:

| Company | Year | TotalIncome | Profit | EmployeeCount | NetPayCosts | AveragePay |
|---------|------|-------------|--------|---------------|-------------|------------|
| Codeus  | 2020 | 221152      | 91040  | 13            | 64418       | 412.9359   |
| Codeus  | 2019 | 131904      | 41568  | 12            | 0           | 0          |

...

Za godine prije 2020 ne postoji podatak o neto troškovima zarade, pa je ta vrijednost = 0 (kao i kalkulisana vrijednost prosječne zarade).

## Rust implementacija

Projekat takođe uključuje Rust implementaciju sa istom funkcionalnošću kao i PowerShell skripta.

### Zahtjevi

- Rust (https://www.rust-lang.org/tools/install)

### Kompilacija

Za kompilaciju projekta, pokrenite sljedeću komandu iz root direktorija projekta:

```bash
cargo build --release
```

Ova komanda će generisati binarne fajlove u `target/release/` direktoriju.

### Pokretanje

Projekat uključuje dvije binarne verzije koje se mogu pokrenuti:

1. **main** - Prikuplja podatke za sve predefinisane kompanije (slično PowerShell skripti):
   ```bash
   cargo run --release
   ```

2. **specific** - Prikuplja podatke samo za jednu kompaniju (potrebno modificirati `src/specific.rs` i dodati PIB i naziv kompanije):
   ```bash
   cargo run --release --bin specific
   ```

Za pokretanje binarnih fajlova direktno (nakon kompilacije):
```bash
./target/release/scraping_eprijava    # Za main verziju
./target/release/specific             # Za specific verziju
```

### Konfiguracija

Za `specific` verziju, potrebno je modificirati `src/specific.rs` i dodati PIB i naziv kompanije u hash mapu:
```rust
let mut companies = HashMap::new();
companies.insert("INSERT_PIB", "INSERT_COMPANY_NAME");
```

Takođe, možda će biti potrebno ažurirati session cookie u oba fajla ako je istekao:
```rust
let session_cookie = "taxisSession=ir3pdvm0e20di2u4p2dfh4d4"; // IMPORTANT: Update this if needed
```
