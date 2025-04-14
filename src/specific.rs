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
    companies.insert("INSERT_PIB", "INSERT_COMPANY_NAME");

    // --- Setup CSV ---
    let csv_path = Path::new("./Results.csv");
    let csv_file = File::create(csv_path)?;
    let mut csv_writer = WriterBuilder::new().has_headers(false).from_writer(csv_file);
    // Write header manually to match PowerShell script exactly
    csv_writer.write_record(&["name", "Year", "totalIncome", "profit", "employeeCount", "netPayCosts", "averagePay"])?;
    csv_writer.flush()?; // Ensure header is written immediately
    info!("CSV file initialized: {}", csv_path.display());

    // --- HTTP Client ---
    let client = Client::builder()
        .timeout(Duration::from_secs(60)) // Add a timeout
        .build()?;
    info!("HTTP Client initialized.");

    // --- Session Cookie (Needs to be updated manually if expired) ---
    let session_cookie = "taxisSession=ir3pdvm0e20di2u4p2dfh4d4"; // IMPORTANT: Update this if needed
    info!("Using session cookie: {}", session_cookie); // Be mindful if logging sensitive info

    // --- Process Each Company ---
    for (pib, company_name) in &companies {
        info!("\\nProcessing Company: {} ({})", company_name, pib);

        // --- Create Company Sub-folder ---
        let company_folder = Path::new(company_name);
        match create_dir_all(company_folder) {
            Ok(_) => info!("Created/verified directory: {}", company_folder.display()),
            Err(e) => {
                error!("Failed to create directory {}: {}. Skipping company.", company_folder.display(), e);
                continue; // Skip company on error
            }
        };

        // --- Get List of Financial Reports ---
        info!("\\nFetching list of financial reports...");
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

        // This debug log remains crucial for diagnosing JSON parsing errors
        debug!("Raw response for {}: {}", company_name, response_text);

        let details_json: DetailsResponse = match serde_json::from_str(&response_text) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to parse JSON report list for {}: {}\\nResponse Text: {}", company_name, e, response_text);
                continue;
            }
        };
        info!("Successfully fetched {} reports for {}.", details_json.data.len(), company_name);

        // --- Process Each Financial Report ---
        for report in details_json.data {
            info!("Processing report for Year: {} (ID: {})", report.year, report.fin_statement_number);

            let report_url = format!(
                "https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/GetStatement?id={}",
                report.fin_statement_number
            );

            info!("Fetching HTML report from: {}", report_url);
            let report_response_result = client
                .get(&report_url)
                .header(COOKIE, session_cookie)
                .send();

            let report_response = match report_response_result {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to fetch report {} for {}: {}", report.fin_statement_number, company_name, e);
                    continue; // Skip this report
                }
            };

            let report_content = match report_response.text() {
                 Ok(text) => text,
                 Err(e) => {
                     error!("Failed to read report content for {} ({}): {}", company_name, report.year, e);
                     continue; // Skip this report
                 }
            };
            info!("Successfully downloaded HTML report for {} ({})", company_name, report.year);
            debug!("HTML Report content length: {}", report_content.len());


            // --- Extract Data ---
            debug!("Attempting to parse values from HTML for {} ({})", company_name, report.year);

            // Try original regex first
            let mut total_income = parse_html_value(&RE_TOTAL_INCOME_ORIGINAL, &report_content, "totalIncome");
            debug!("Parsed total income (original regex): {}", total_income);

            // If original fails (returns 0), try the new regex
            if total_income == 0 {
                total_income = parse_html_value(&RE_TOTAL_INCOME_NEW, &report_content, "totalIncome");
                 debug!("Parsed total income (new regex): {}", total_income);
                 if total_income == 0 {
                     warn!("Could not parse total income for {} ({})", company_name, report.year);
                 }
            }

            let profit = parse_html_value(&RE_PROFIT, &report_content, "profit");
            debug!("Parsed profit: {}", profit);
            if profit == 0 { // Added warning for profit as well, as it's a key metric
                warn!("Could not parse profit (or profit is zero) for {} ({})", company_name, report.year);
            }

            let employee_count = parse_html_value(&RE_EMPLOYEE_COUNT, &report_content, "employeeCount");
            debug!("Parsed employee count: {}", employee_count);

            let net_pay_costs = parse_html_value(&RE_NET_PAY_COSTS, &report_content, "netPayCosts");
            debug!("Parsed net pay costs: {}", net_pay_costs);


            let average_pay = if employee_count > 0 {
                (net_pay_costs as f64) / (employee_count as f64) / 12.0 // Assuming monthly average
            } else {
                0.0
            };
            debug!("Calculated average pay: {:.2}", average_pay);


            // --- Create CSV Record ---
            let record = CsvRecord {
                name: company_name.to_string(),
                year: report.year.clone(),
                total_income,
                profit,
                employee_count,
                net_pay_costs,
                average_pay,
            };
             debug!("Created CSV record: {:?}", record);

            // --- Write to CSV ---
            csv_writer.serialize(&record)?;
            csv_writer.flush()?; // Flush after each record to see progress
            info!("Written record to CSV for {} ({})", company_name, report.year);


            // --- Save HTML Report ---
            let html_filename = format!("{}_{}.html", company_name, report.year);
            let html_path = company_folder.join(html_filename);
            match fs::write(&html_path, &report_content) {
                Ok(_) => info!("Saved HTML report to: {}", html_path.display()),
                Err(e) => error!("Failed to save HTML report {} for {}: {}", html_path.display(), company_name, e),
            };


            // --- Polite Delay ---
            debug!("Sleeping for 1 second...");
            thread::sleep(Duration::from_secs(1));
        }
        info!("Finished processing all reports for {}", company_name);
    }

    info!("Scraping process completed successfully.");
    Ok(())
}