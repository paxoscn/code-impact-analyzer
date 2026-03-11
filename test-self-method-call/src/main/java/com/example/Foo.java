package com.example;

/**
 * 测试类：验证类对自身方法调用的解析
 */
public class Foo {
    
    // 实例方法
    public void bar() {
        System.out.println("Instance method bar");
    }
    
    // 静态方法
    public static void staticBar() {
        System.out.println("Static method staticBar");
    }
    
    // 测试方法：包含三种调用方式
    public void testMethodCalls() {
        // 场景1: 直接调用实例方法
        bar();
        
        // 场景2: 使用this显式调用实例方法
        this.bar();
        
        // 场景3: 使用类名调用静态方法
        Foo.staticBar();
        
        // 场景4: 直接调用静态方法
        staticBar();
    }
    
    // 另一个方法用于测试
    public void anotherMethod() {
        bar();
        this.bar();
        Foo.staticBar();
    }
}
