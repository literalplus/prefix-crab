```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update
helm upgrade --install --namespace=grafana -f values.yaml --create-namespace grafana grafana/grafana
```

https://github.com/helm/helm/issues/6261#issuecomment-523472128

if you use this annotation on the PVC, it will skip deleting the resource on uninstall. I think it has been retained from v2

helm.sh/resource-policy: "keep"

Docs: https://helm.sh/docs/charts_tips_and_tricks/#tell-tiller-not-to-delete-a-resource
Source code In dev-v3