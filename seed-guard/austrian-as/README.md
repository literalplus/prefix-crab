# austrian-as

Data source: https://whois.ipinsight.io/countries/AT

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python3 scrape.py
```

This generates a CSV file. Open it with your favourite spreadsheet software and filter it to your likings.

Filter it if needed and save again as a CSV.

You can now reduce it to two columns: ASN and description & import it into the as_filter_list table using
a tool such a DBeaver. Make sure to set the mode to allowlist and not denylist.
 