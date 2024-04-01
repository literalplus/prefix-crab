library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)
library(stringr)

dat_uni <- read_csv("2024-03-21_university48s.csv")
pfx_merged <- read_csv("uni_merged_tree.csv")
pfx_merged_raw <- read_csv("uni_merged_tree_raw.csv")
pfx_u3 <- read_csv("prefix_tree_u3.csv")

NET_BREAKS <- c(29, 32, 40, 48, 56, 64)

###

hist(dat_uni$received_count) # nearly all 16, some 15

ggplot(data = dat_uni, aes(last_hop_routers)) +
  geom_bar(color = "black") +
  coord_flip()

netdist_raw <- ggplot(data = pfx_merged_raw |> filter(is_leaf), aes(net_len)) +
  geom_histogram(fill = "#d29630", color = "black", binwidth = 1) +
  labs(title = NULL, x = "Prefix Length") +
  scale_y_continuous(name = "# Leaves (Raw)") +
  scale_x_continuous()

netdist_clean <- ggplot(data = pfx_merged |> filter(is_leaf), aes(net_len)) +
  geom_histogram(fill = "#d29630", color = "black", binwidth = 1) +
  labs(title = NULL, x = "Prefix Length") +
  scale_y_continuous(name = "# Leaves (Cleaned)") +
  scale_x_continuous()

ggsave(
  "Eval-Univ-RawNetDist.pdf",
  ggarrange(
    netdist_raw, netdist_clean, ncol=2
  ),
  units="cm", width=19, height=6
)

### figure out which LHRs to ignore

unique(pfx_merged_raw$last_hop_routers) 
# B -> "unused net" / likely a border router on their side
# L -> next hop upstream, likely rate limiting
# U -> their own net
# [1] NA                                                              
# [2] "B:1101:1007::1;L:c1c:804a::2"                    
# [3] "B:1101:1007::1"                                         
# [4] "B:1101:1007::1;U:1:53::dc:2;L:c1c:804a::2"
# [5] "U:100:ffff::2"                                          
# [6] "U:103:1::feed"                                          
# [7] "U:104::1"                                               
# [8] "U:107:1::1" 

###
