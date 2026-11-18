import os
from pathlib import Path as P

# Keep this business constraint.
def calculate(items, limit=3):
    total = 0
    for item in items:
        if item > limit:
            total += item
    return total

class Repository(Base):
    def load(self, key):
        return P(key)
