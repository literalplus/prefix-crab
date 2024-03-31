Inspired by https://github.com/traefik/traefik-helm-chart but that uses
a Deployment instead of a DaemonSet

```bash
helm dependency update
helm upgrade noatnet-ingress . --namespace=kube-ingress --create-namespace --install
kubectl apply -f ingress-fallback.yaml -n shared
```
