# Deklarisanje niza kompanija
$companies = New-Object "System.Collections.Generic.Dictionary[[String], [String]]"
$companies.Add("03014215", "Coinis")
$companies.Add("02686473", "Domen")
$companies.Add("02775018", "CoreIT")
$companies.Add("02632284", "Logate")
$companies.Add("02783061", "Bild Studio")
$companies.Add("02907259", "Amplitudo")
$companies.Add("03073572", "Datum Solutions")
$companies.Add("02713098", "Poslovna Inteligencija")
$companies.Add("03037258", "International Bridge")
$companies.Add("02731517", "Fleka")
$companies.Add("02679744", "Datalab")
$companies.Add("03167453", "Omnitech")
$companies.Add("03131343", "SynergySuite")
$companies.Add("03122123", "Alicorn")
$companies.Add("03066258", "Codingo")
$companies.Add("03274357", "Uhura Solutions")
$companies.Add("02246244", "Winsoft")
$companies.Add("02177579", "Cikom")
$companies.Add("02961717", "Media Monkeys")
$companies.Add("03091627", "Codeus")
$companies.Add("03084434", "Digital Control")
$companies.Add("03165663", "Ridgemax")
$companies.Add("03360962", "Infinum")
$companies.Add("03191451", "Kodio")
$companies.Add("03381447", "EPAM")
$companies.Add("03413772", "First Line Software")
$companies.Add("03374700", "Vega IT Omega")
$companies.Add("03373398", "Quantox Technology")
$companies.Add("03216446", "Ooblee")
$companies.Add("03209296", "BIXBIT")
$companies.Add("03367053", "GoldBear Technologies")
$companies.Add("03421198", "G5 Entertainment")
$companies.Add("03428184", "Tungsten Montenegro")
$companies.Add("03110222", "BGS Consulting")
$companies.Add("03413381", "Artec 3D Adriatica")
$companies.Add("03413616", "Customertimes Montenegro")
$companies.Add("03200116", "Codepixel")
$companies.Add("03403912", "Codemine")
$companies.Add("03418545", "Belka")
$companies.Add("03489159", "Playrix")
$companies.Add("03424804", "FSTR")
$companies.Add("03442586", "Arctic 7")

# Formiranje CSV fajla za smjestanje rezultata
Set-Content -Path "./Results.csv" -Value '"name","Year","totalIncome","profit","employeeCount","netPayCosts","averagePay"'

# Definisanje header-a zbog provizornog ID-a sesije
$headers = New-Object "System.Collections.Generic.Dictionary[[String],[String]]"
$headers.Add("Cookie", "taxisSession=ir3pdvm0e20di2u4p2dfh4d4")

foreach ($company in $companies.GetEnumerator()) {
	Write-Host "`nPrikupljanje podataka za: $($company.Value) ($($company.Key))"

	# Pretraga pravnog lica po PIB-u na portalu ePrijava
	$pib = $company.Key
	$response = Invoke-RestMethod "https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/Grid?pib=$($pib)&naziv=&orderBy=naziv&skip=0&take=1" -Method 'POST' -Headers $headers
	$taxpayers = $response.TaxPayerRows

	# Pronadjena sljedeca pravna lica
	foreach ($taxpayer in $taxpayers) {
		Write-Host "Pronadjen: $($taxpayer.PIB) - $($taxpayer.Naziv)"
	}

	# Kreiranje pod-foldera za pravno lice
	New-Item -ItemType Directory -Force -Path "./$($company.Value)\" | Out-Null

	# Detalji pravnog lica
	Write-Host "`nDownload detalja pravnog lica"
	$response = Invoke-RestMethod "https://eprijava.tax.gov.me/TaxisPortal/TaxPayerCompanies/Details?PIB=$($pib)" -Method 'POST' -Headers $headers
	Out-File -FilePath "./$($company.Value)\$($pib).htm" -InputObject $response -Encoding UTF8

	# Pretraga liste finansijskih izvjestaja
	Write-Host "`nPretraga liste finansijskih izvjestaja"
	$response = Invoke-RestMethod "https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/TaxPayerStatementsList?PIB=$($pib)&take=20&skip=0&page=1&pageSize=20" -Method 'POST' -Headers $headers
	$finStatements = $response.data

	# Pronadjeni sljedeci finansijski izvjestaji
	Write-Host "Pronadjeno $($finStatements.length) finansijskih izvjestaja"
	# $finStatements

	# Download svakog pronadjenog finansijskog izvjestaja
	Write-Host "`nDownload finansijskih izvjestaja..."
	foreach ($finStatement in $finStatements) {
		$no = $finStatement.FinStatementNumber
		$year = $finStatement.Year
		Write-Host "Download izvjestaja br. $($no) za godinu $($year)"

		# Define the target file path
		$targetFilePath = "./$($company.Value)/$($pib)-$($year).html"

		# Check if the file already exists locally
		if (Test-Path $targetFilePath) {
			Write-Host "File $targetFilePath already exists locally. Skipping download." -ForegroundColor Green
		} else {
			# Download the report if it doesn't exist
			$response = Invoke-RestMethod "https://eprijava.tax.gov.me/TaxisPortal/FinancialStatement/Details?rbr=$($no)" -Method 'POST' -Headers $headers

			# Save the report
			Out-File -FilePath $targetFilePath -InputObject $response -Encoding UTF8
		}

		Write-Host "`nIme firme u obradi u sledecem redu"
		Write-Host $company.Value
		Write-Host $targetFilePath

		$imeFirme = $company.Value

		# Read content from the (potentially pre-existing) local file
		$content = [IO.File]::ReadAllText($targetFilePath)

		# Pretraga podatka: totalIncome
		$totalIncome = 0
		# Original regex pattern
		$patternOriginal = '<td style="text-align: center;">201<\/td>\s*<td><\/td>\s*<td style="text-align: right; padding-right: 8px">(?<totalIncome>\d+)<\/td>'
		# New regex pattern (for structures like Domen's report)
		$patternNew = '<tr>\s*<td.*?>.*?<\/td>\s*<td.*?>.*?<\/td>\s*<td style="text-align: center;">201<\/td>\s*<td.*?>.*?<\/td>\s*<td style="text-align: right; padding-right: 8px">(?<totalIncome>\d+)<\/td>'

		# Try original pattern first
		$result = [regex]::Matches($content, $patternOriginal)

		if ($result -ne $null -and $result.Count -gt 0 -and ($result[0].Groups['totalIncome'].Value -as [int]) -gt 0) {
			$totalIncome = $result[0].Groups['totalIncome'].Value -as [int]
		} else {
			# If original pattern failed or gave 0, try the new pattern
			Write-Host "Original pattern failed for totalIncome, trying new pattern..." -ForegroundColor Yellow
			$result = [regex]::Matches($content, $patternNew)
			if ($result -ne $null -and $result.Count -gt 0) {
				$totalIncome = $result[0].Groups['totalIncome'].Value -as [int]
			}
		}

		# Pretraga podatka: profit
		$profit = 0
		$pattern = '<td style="text-align: left">IX. Neto sveobuhvatni rezultat \(248\+259\)<\/td>\s*<td style="text-align: center;">260<\/td>\s*<td><\/td>\s*<td style="text-align: right; padding-right: 8px">(?<profit>\d+)<\/td>'

		$result = [regex]::Matches($content, $pattern)
		if ($result -ne $null) {
			$profit = $result[0].Groups['profit'].Value -as [int]
		}

		# Pretraga podatka: employeeCount
		$employeeCount = 0
		$pattern = '<td style="text-align: left">Prosje\?an broj zaposlenih \(ukupan broj zaposlenih krajem svakog mjeseca podijeljen sa brojem mjeseci\)<\/td>\s*<td style="text-align: center;">001<\/td>\s*<td><\/td>\s*<td style="text-align: right; padding-right: 8px">(?<employeeCount>\d+)<\/td>'

		$result = [regex]::Matches($content, $pattern)
		if ($result -ne $null) {
			$employeeCount = $result[0].Groups['employeeCount'].Value -as [int]
		}

		Write-Host 'podaci ucitani - - -- -- - -- -- -'
		Write-Host $totalIncome
		Write-Host $employeeCount

		# Pretraga podatka: netPayCosts i kalkulacija averagePay
		$netPayCosts = 0
		$averagePay = 0

		$pattern = '<td style="text-align: left">a\) Neto troškovi zarada, naknada zarada i lični rashodi<\/td>\s*<td style="text-align: center;">212<\/td>\s*<td><\/td>\s*<td style="text-align: right; padding-right: 8px">(?<netPayCosts>\d+)<\/td>'
		$result = [regex]::Matches($content, $pattern)
		if ($result -ne $null) {
			$netPayCosts = $result[0].Groups['netPayCosts'].Value -as [int]
			$averagePay = $netPayCosts / $employeeCount / 12
		}

		# Upis rezultata u Results.csv fajl
		Add-Content -Path "./Results.csv" -Value """$($company.Value)"", $($year), $($totalIncome), $($profit), $($employeeCount), $($netPayCosts), $($averagePay)"

	}

}

Write-Host "`nGotovo."
