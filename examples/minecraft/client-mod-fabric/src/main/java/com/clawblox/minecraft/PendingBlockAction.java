package com.clawblox.minecraft;

final class PendingBlockAction {
    final String kind;
    final long expiresTick;

    PendingBlockAction(String kind, long expiresTick) {
        this.kind = kind;
        this.expiresTick = expiresTick;
    }
}
