"""Sample Python module for testing."""
import os

class Calculator:
    """A simple calculator."""
    def add(self, a, b):
        return a + b

    def multiply(self, a, b):
        return a * b

def main():
    calc = Calculator()
    result = calc.add(1, 2)
    print(result)
