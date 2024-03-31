just a little guy that collects logs

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update
helm upgrade --install --namespace=grafana -f values.yaml --create-namespace loki grafana/loki
```
