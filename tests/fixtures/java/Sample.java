import java.util.List;

public class Sample {
    private String name;

    public Sample(String name) {
        this.name = name;
    }

    public void print() {
        System.out.println(name);
    }

    public static void main(String[] args) {
        Sample s = new Sample("hello");
        s.print();
    }
}
