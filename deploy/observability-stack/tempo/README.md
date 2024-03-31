https://github.com/grafana/helm-charts/tree/main/charts/tempo

trace DB for grafana

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm upgrade --install --namespace=grafana -f values.yaml tempo grafana/tempo
helm show values grafana/tempo >default-values.yaml
```