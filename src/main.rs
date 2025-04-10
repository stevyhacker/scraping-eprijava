use anyhow::{Context, Result};
use csv::WriterBuilder;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, COOKIE, CONTENT_LENGTH};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs::{self, create_dir_all, File};
use std::path::Path;
use std::thread;
use std::time::Duration;

// --- Structs for Deserialization ---

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct GridResponse {
    tax_payer_rows: Vec<TaxPayerRow>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct TaxPayerRow {
    pib: String,
    naziv: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct FinancialStatement {
    fin_statement_number: String, // Corresponds to Rbr in PS script
    year: String,
}

#[derive(Serialize, Debug)]
struct CsvRecord {
    name: String,
    year: String,
    total_income: i64,
    profit: i64,
    employee_count: i64,
    net_pay_costs: i64,
    average_pay: f64,
}

#[derive(Deserialize, Debug)]
struct DetailsResponse {
    data: Vec<FinancialStatement>,
}

// --- Regex Definitions (Lazy Static for efficiency) ---

static RE_TOTAL_INCOME_ORIGINAL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<td style="text-align: center;">201</td>\s*<td></td>\s*<td style="text-align: right; padding-right: 8px">(?<totalIncome>\d+)</td>"#).unwrap()
});

static RE_TOTAL_INCOME_NEW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<tr>\s*<td.*?>.*?</td>\s*<td.*?>.*?</td>\s*<td style="text-align: center;">201</td>\s*<td.*?>.*?</td>\s*<td style="text-align: right; padding-right: 8px">(?<totalIncome>\d+)</td>"#).unwrap()
});

static RE_PROFIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<td style="text-align: left">IX\. Neto sveobuhvatni rezultat \(248\+259\)</td>\s*<td style="text-align: center;">260</td>\s*<td></td>\s*<td style="text-align: right; padding-right: 8px">(?<profit>\d+)</td>"#).unwrap()
});

static RE_EMPLOYEE_COUNT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<td style="text-align: left">Prosje[^<]+an broj zaposlenih[^<]+</td>\s*<td style="text-align: center;">001</td>\s*<td></td>\s*<td style="text-align: right; padding-right: 8px">(?<employeeCount>\d+)</td>"#).unwrap()
});

static RE_NET_PAY_COSTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<td style="text-align: left">a\) Neto troškovi zarada, naknada zarada i lični rashodi</td>\s*<td style="text-align: center;">212</td>\s*<td></td>\s*<td style="text-align: right; padding-right: 8px">(?<netPayCosts>\d+)</td>"#).unwrap()
});

// --- Helper Functions ---

fn parse_html_value(re: &Regex, content: &str, capture_name: &str) -> i64 {
    re.captures(content)
        .and_then(|caps| caps.name(capture_name))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(0)
}

// --- Main Logic ---

fn main() -> Result<()> {
    env_logger::init(); // Initialize logger

    // --- Company List (from PowerShell script) ---
    let mut companies = HashMap::new();
    companies.insert("03014215", "Coinis");
    companies.insert("02686473", "Domen");
    companies.insert("02775018", "CoreIT");
    companies.insert("02632284", "Logate");
    companies.insert("02783061", "Bild Studio");
    companies.insert("02907259", "Amplitudo");
    companies.insert("03073572", "Datum Solutions");
    companies.insert("02713098", "Poslovna Inteligencija"); // Updated PIB
    companies.insert("03037258", "International Bridge");
    companies.insert("02731517", "Fleka");
    companies.insert("02679744", "Datalab");
    companies.insert("03167453", "Omnitech");
    companies.insert("03131343", "SynergySuite");
    companies.insert("03122123", "Alicorn"); // Updated PIB
    companies.insert("03066258", "Codingo");
    companies.insert("03274357", "Uhura Solutions");
    companies.insert("02246244", "Winsoft");
    companies.insert("02177579", "Cikom");
    companies.insert("02961717", "Media Monkeys"); // Updated PIB
    companies.insert("03091627", "Codeus");
    companies.insert("03084434", "Digital Control");
    companies.insert("03165663", "Ridgemax");
    companies.insert("03360962", "Infinum");
    companies.insert("03191451", "Kodio");
    companies.insert("03381447", "EPAM");
    companies.insert("03413772", "First Line Software");
    companies.insert("03374700", "Vega IT Omega");
    companies.insert("03373398", "Quantox Technology");
    companies.insert("03216446", "Ooblee");
    companies.insert("03209296", "BIXBIT");
    companies.insert("03367053", "GoldBear Technologies");
    companies.insert("03421198", "G5 Entertainment");
    companies.insert("03428184", "Tungsten Montenegro");
    companies.insert("03110222", "BGS Consulting");
    companies.insert("03413381", "Artec 3D Adriatica");
    companies.insert("03413616", "Customertimes Montenegro");
    companies.insert("03200116", "Codepixel");
    companies.insert("03403912", "Codemine");
    companies.insert("03418545", "Belka");
    companies.insert("03489159", "Playrix");
    companies.insert("03424804", "FSTR");
    companies.insert("03442586", "Arctic 7");

    // --- Setup CSV ---
    let csv_path = Path::new("./Results.csv");
    let csv_file = File::create(csv_path)?;
    let mut csv_writer = WriterBuilder::new().has_headers(false).from_writer(csv_file);
    // Write header manually to match PowerShell script exactly
    csv_writer.write_record(&["name", "Year", "totalIncome", "profit", "employeeCount", "netPayCosts", "averagePay"])?;
    csv_writer.flush()?; // Ensure header is written immediately

    // --- HTTP Client ---
    let client = Client::builder()
        .timeout(Duration::from_secs(60)) // Add a timeout
        .build()?;

    // --- Session Cookie (Needs to be updated manually if expired) ---
    let session_cookie = "taxisSession=ir3pdvm0e20di2u4p2dfh4d4"; // IMPORTANT: Update this if needed

    // --- Process Each Company ---
    for (pib, company_name) in &companies {
        info!("\nPrikupljanje podataka za: {} ({})", company_name, pib);

        // --- Create Company Sub-folder ---
        let company_folder = Path::new(company_name);
        create_dir_all(company_folder).context(format!("Failed to create directory: {}", company_folder.display()))?;

        // --- Find Taxpayer Info (Simplified - assumes first result is correct) ---
        // let grid_url = format!("https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/Grid?pib={}&naziv=&orderBy=naziv&skip=0&take=1", pib);
        // let grid_response = client
        //     .post(&grid_url)
        //     .header("Cookie", session_cookie)
        //     .send()?
        //     .json::<GridResponse>()?;

        // if let Some(taxpayer) = grid_response.tax_payer_rows.first() {
        //     info!("Pronadjen: {} - {}", taxpayer.pib, taxpayer.naziv);
        // } else {
        //     warn!("Nije pronadjeno pravno lice za PIB: {}", pib);
        //     continue; // Skip to next company
        // }
        // Note: Skipping the grid lookup as the PowerShell script doesn't seem to use the result beyond logging

        // --- Get List of Financial Reports ---
        info!("\nPretraga liste finansijskih izvjestaja");
        // Corrected URL based on PowerShell script
        let details_list_url = format!("https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/TaxPayerStatementsList?PIB={}&take=20", pib);
        let details_response_result = client
            .post(&details_list_url)
            .header(COOKIE, session_cookie)
            .header(CONTENT_LENGTH, "0")
            .header(ACCEPT, "application/json")
            .send();

        let details_response = match details_response_result {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to get report list for {}: {}", company_name, e);
                continue; // Skip company on error
            }
        };

        // Log the raw response text first to debug parsing issues
        let response_text = match details_response.text() {
            Ok(text) => text,
            Err(e) => {
                error!("Failed to read response text for {}: {}", company_name, e);
                continue;
            }
        };
        debug!("Raw response for {}: {}", company_name, response_text);

        // Now try to parse the logged text as JSON
        let reports: Vec<FinancialStatement> = match serde_json::from_str::<DetailsResponse>(&response_text) {
            Ok(data) => data.data,
            Err(e) => {
                error!("Failed to parse report list JSON for {}: {}", company_name, e);
                continue; // Skip company on error
            }
        };

        info!("Pronadjeno {} finansijskih izvjestaja", reports.len());

        // --- Process Each Financial Report ---
        for report in reports {
            let rbr = &report.fin_statement_number;
            let year = &report.year;
            info!("Processing report no. {} for year {}", rbr, year);

            // Construct local file path
            let local_file_path_str = format!("{}/{}-{}.html", company_folder.display(), pib, year);
            let local_file_path = Path::new(&local_file_path_str);

            let report_html: String;

            // Check if file exists locally
            if local_file_path.exists() {
                info!("File {} already exists locally. Reading from disk.", local_file_path.display());
                report_html = match fs::read_to_string(local_file_path) {
                    Ok(content) => content,
                    Err(e) => {
                        error!("Failed to read local file {}: {}. Skipping report.", local_file_path.display(), e);
                        continue;
                    }
                };
            } else {
                 info!("Downloading report {} for year {} to {}", rbr, year, local_file_path.display());
                 // Download the report details HTML
                 let report_url = format!("https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/Details?rbr={}", rbr);
                 // The '?' operator handles the Result from .text() and .send()
                 // If successful, report_html_result contains the String
                 let report_html_result = client
                      .post(&report_url)
                      .header(COOKIE, session_cookie)
                      .header(CONTENT_LENGTH, "0")
                      .send()
                      .context("Failed to send request for report details")
                      .and_then(|res| res.text().context("Failed to read report details response text"));

                 // Handle the result of the download and text extraction
                 match report_html_result {
                     Ok(html) => {
                         // Successfully got the HTML, save it
                         if let Err(e) = fs::write(local_file_path, &html) {
                             warn!("Failed to save downloaded file {}: {}", local_file_path.display(), e);
                         }
                         report_html = html; // Assign the valid HTML
                     },
                     Err(e) => {
                         error!("Failed to download or read report details for {}: {}. Skipping report.", company_name, e);
                         continue; // Skip report on download/read error
                     }
                 }
            }

            // --- Extract Data using Regex ---
            let mut total_income = parse_html_value(&RE_TOTAL_INCOME_ORIGINAL, &report_html, "totalIncome");
            if total_income == 0 {
                 warn!("Original pattern failed for totalIncome for {} ({}), trying new pattern...", company_name, year);
                 total_income = parse_html_value(&RE_TOTAL_INCOME_NEW, &report_html, "totalIncome");
                 if total_income == 0 {
                     warn!("New pattern also failed for totalIncome for {} ({})", company_name, year);
                 }
            }

            let profit = parse_html_value(&RE_PROFIT, &report_html, "profit");
            let employee_count = parse_html_value(&RE_EMPLOYEE_COUNT, &report_html, "employeeCount");
            let net_pay_costs = parse_html_value(&RE_NET_PAY_COSTS, &report_html, "netPayCosts");

            let average_pay = if employee_count > 0 {
                (net_pay_costs as f64) / (employee_count as f64) / 12.0 // Assuming monthly average
            } else {
                0.0
            };

            info!(
                "podaci ucitani - totalIncome: {}, profit: {}, employees: {}, netPayCosts: {}",
                total_income, profit, employee_count, net_pay_costs
            );

            // --- Write Record to CSV ---
             let record = CsvRecord {
                 name: company_name.to_string(),
                 year: year.clone(), // Clone the String year
                 total_income,
                 profit,
                 employee_count,
                 net_pay_costs,
                 average_pay,
             };

            if let Err(e) = csv_writer.serialize(record) {
                 error!("Failed to write CSV record for {} ({}): {}", company_name, year, e);
            }

             // Small delay to avoid overwhelming the server
             thread::sleep(Duration::from_millis(200));
        }
        csv_writer.flush()?; // Flush after each company
    }

    info!("\nGotovo.");
    Ok(())
}
