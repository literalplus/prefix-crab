library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)
library(stringr)

dat_uni <- read_csv("2024-03-21_university48s.csv")
pfx_merged <- read_csv("uni_merged_tree.csv")
pfx_u3 <- read_csv("prefix_tree_u3.csv")

###

hist(dat_uni$received_count) # nearly all 16, some 15

barplot(dat_uni$last_hop_routers)

ggplot(data = dat_uni, aes(last_hop_routers)) +
  geom_bar(color = "black") +
  coord_flip()

###
