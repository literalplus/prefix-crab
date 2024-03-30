library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)

at10 <- read_csv("2024-03-27_at10-hit-counts.csv")
at11 <- read_csv("2024-03-27_at11-hit-counts.csv")

percent_of <- function(label, mine, base) {
  paste0(label, " (none: ", formatC(100 * ((nrow(base)-nrow(mine))/nrow(base)), format = "f", digits = 0), "%)")
}


###

at10_err <- at10 %>%
  mutate(err_rate = zmap_received_err / zmap_sent) %>%
  filter(err_rate > 0)
at11_err <- at11 %>%
  mutate(err_rate = zmap_received_err / zmap_sent) %>%
  filter(err_rate > 0)

err_ratio_at10 <- ggplot(data = at10_err, aes(x = err_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#9967bc", bins = 10) + 
  labs(title = NULL, x = percent_of("AT-10", at10_err, at10)) +
  coord_cartesian(ylim = c(0, 0.4)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 0.6, 0.2)) +
  geom_vline(aes(xintercept=median(err_rate)), color="#34b0a7")

err_ratio_at11 <- ggplot(data = at11_err, aes(x = err_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#d19cf5", bins = 10) + 
  labs(title = NULL, x = percent_of("AT-11", at11_err, at11)) +
  coord_cartesian(ylim = c(0, 0.4)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 0.6, 0.2)) +
  geom_vline(aes(xintercept=median(err_rate)), color="#34b0a7")

err_ratios <- ggarrange(err_ratio_at10, err_ratio_at11, ncol = 2)
#err_ratios <- annotate_figure(err_ratios, top = text_grob("zmap: errors received / packets sent", size = 14))
ggsave("Eval-HitCounts-ZmapErrorRates.pdf", err_ratios, units="cm", width=19, height=6)

###

at10 <- at10 %>%
  mutate(echo_rate = zmap_received_echo / zmap_sent)
at11 <- at11 %>%
  mutate(echo_rate = zmap_received_echo / zmap_sent)

echo_at10 <- ggplot(data = at10, aes(x = echo_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#9967bc", bins = 10) + 
  labs(title = NULL, x = "AT-10") +
  coord_cartesian(ylim = c(0, 0.6)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 0.6, 0.2))

echo_at11 <- ggplot(data = at11, aes(x = echo_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#d19cf5", bins = 10) + 
  labs(title = NULL, x = "AT-11") +
  coord_cartesian(ylim = c(0, 0.6)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 0.6, 0.2))

echo <- ggarrange(echo_at10, echo_at11, ncol = 2)
#echo <- annotate_figure(echo, top = text_grob("zmap: echoes received / packets sent", size = 14))
ggsave("Eval-HitCounts-ZmapEchoRate.pdf", echo, units="cm", width=19, height=6)


###

at10_inp <- at10 %>%
  mutate(in_prefix_rate = yarrp_in_prefix / yarrp_sent) %>%
  filter(in_prefix_rate > 0)

at11_inp <- at11 %>%
  mutate(in_prefix_rate = yarrp_in_prefix / yarrp_sent) %>%
  filter(in_prefix_rate > 0)

at11_max_in_prefix <- filter(at11_inp,in_prefix_rate > 0.9)

in_prefix_at10 <- ggplot(data = at10_inp, aes(x = in_prefix_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#9967bc", bins = 20) + 
  labs(title = element_blank(), x = percent_of("AT-10", at10_inp, at10)) +
  coord_cartesian(ylim = c(0, 0.5)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 1, 0.2)) +
  geom_vline(aes(xintercept=median(in_prefix_rate)), color="#34b0a7")

in_prefix_at11 <- ggplot(data = at11_inp, aes(x = in_prefix_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#d19cf5", bins = 20) + 
  labs(title = element_blank(), x = percent_of("AT-11", at11_inp, at11)) +
  coord_cartesian(ylim = c(0, 0.5)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 1, 0.2)) +
  geom_vline(aes(xintercept=median(in_prefix_rate)), color="#34b0a7")

in_prefix <- ggarrange(in_prefix_at10, in_prefix_at11, ncol = 2)
ggsave("Eval-HitCounts-YarrpInPrefix.pdf", in_prefix, units="cm", width=19, height=6)

###

at10_missed <- at10 %>%
  mutate(yarrp_miss_rate = yarrp_missed / yarrp_sent) %>%
  filter(yarrp_missed > 0)

at11_missed <- at11 %>%
  mutate(yarrp_miss_rate = yarrp_missed / yarrp_sent) %>%
  filter(yarrp_missed > 0)

no_yarrp_at10 <- ggplot(data = at10_missed, aes(x = yarrp_miss_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#9967bc", bins = 20) + 
  labs(title = element_blank(), x = percent_of("AT-10", at10_missed, at10)) +
  coord_cartesian(ylim = c(0, 0.4)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 1, 0.2)) +
  geom_vline(aes(xintercept=median(yarrp_miss_rate)), color="#34b0a7")

no_yarrp_at11 <- ggplot(data = at11_missed, aes(x = yarrp_miss_rate)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#d19cf5", bins = 20) + 
  labs(title = element_blank(), x = percent_of("AT-11", at11_missed, at11)) +
  coord_cartesian(ylim = c(0, 0.4)) + 
  scale_y_continuous(name = "% BGP Roots", labels = scales::percent, breaks = seq(0, 1, 0.2)) +
  geom_vline(aes(xintercept=median(yarrp_miss_rate)), color="#34b0a7")

no_yarrp <- ggarrange(no_yarrp_at10, no_yarrp_at11, ncol = 2)
ggsave("Eval-HitCounts-YarrpMissed.pdf", no_yarrp, units="cm", width=19, height=6)

# prefixes that are listed in both
no_yarrp_merged <- at10_missed %>%
  inner_join(at11_missed, by=c("net")) %>%
  select(net)

