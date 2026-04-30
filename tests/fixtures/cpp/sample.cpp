#include <iostream>
#include <string>

class Engine {
public:
    Engine(int power) : power_(power) {}
    void start() { std::cout << "Engine started with power " << power_ << std::endl; }
private:
    int power_;
};

int main() {
    Engine e(100);
    e.start();
    return 0;
}
