from bs4 import BeautifulSoup
import csv
import requests
import sys

COUNTRY = 'AT'
r = requests.get(f"https://whois.ipinsight.io/countries/{COUNTRY}")
if r.status_code != 200:
    raise Exception(f"got status code {r.status_code}")

bs4 = BeautifulSoup(r.text, "html.parser")
table = bs4.find("table", "table")  # the latter is the class

with open('all_austrian_as.csv', 'w', newline='') as csvfile:
    writer = csv.writer(csvfile, dialect='excel')
    writer.writerow(('country', 'asn', 'description', 'num_ipv6s'))

    for tr in table.findAll("tr"):
        tds = tr.findAll("td")
        if len(tds) == 0:
            continue
        
        asn_td = tds[0]  # <td><a href="/AS679" title="AS679 - TUNET-AS - Technische Universitat Wien, AT">AS679</a> </td>
        asn = int(asn_td.text.strip().replace("AS", ""))

        name_td = tds[1]  # <td>TUNET-AS - Technische Universitat Wien, AT</td>
        name = name_td.text.strip()

        ipv6_count_td = tds[3]
        ipv6_count = int(ipv6_count_td.text.replace(",", ""))

        if ipv6_count == 0:
            continue
        
        writer.writerow((COUNTRY, asn, f"All AS from {COUNTRY} - {name}", ipv6_count))
