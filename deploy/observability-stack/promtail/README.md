needed for loki to read logs from the nodes

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update
helm upgrade --install --namespace=grafana -f values.yaml --create-namespace loki-promtail grafana/promtail
```
