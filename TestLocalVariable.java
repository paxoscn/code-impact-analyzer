package com.example;

public class TestLocalVariable {
    public void go() {
        Foo foo = new Foo();
        foo.bar();
    }
}

class Foo {
    public void bar() {
        System.out.println("bar called");
    }
}
