https://artifacthub.io/packages/helm/prometheus-community/prometheus

```bash
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm upgrade --install --namespace=grafana -f values.yaml prometheus prometheus-community/prometheus
helm show values prometheus-community/prometheus >default-values.yaml
```