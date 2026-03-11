package com.example.test;

import com.google.common.collect.Lists;
import com.google.common.collect.Sets;
import com.google.common.collect.Maps;
import java.util.*;

public class CollectionFactoryTest {
    
    // 测试方法：接收不同类型的集合参数
    public void processList(List<String> items) {
        System.out.println("Processing list: " + items.size());
    }
    
    public void processSet(Set<String> items) {
        System.out.println("Processing set: " + items.size());
    }
    
    public void processMap(Map<String, String> items) {
        System.out.println("Processing map: " + items.size());
    }
    
    // 测试场景
    public void testGuavaFactories() {
        // Guava Lists - 应该识别为 processList(List)
        processList(Lists.newArrayList());
        processList(Lists.newLinkedList());
        
        // Guava Sets - 应该识别为 processSet(Set)
        processSet(Sets.newHashSet());
        processSet(Sets.newLinkedHashSet());
        
        // Guava Maps - 应该识别为 processMap(Map)
        processMap(Maps.newHashMap());
        processMap(Maps.newLinkedHashMap());
    }
    
    public void testJavaFactories() {
        // Java 9+ List.of - 应该识别为 processList(List)
        processList(List.of("a", "b", "c"));
        
        // Java 9+ Set.of - 应该识别为 processSet(Set)
        processSet(Set.of("x", "y", "z"));
        
        // Java 9+ Map.of - 应该识别为 processMap(Map)
        processMap(Map.of("key1", "value1"));
    }
    
    public void testCollectionsUtility() {
        // Collections.emptyList - 应该识别为 processList(List)
        processList(Collections.emptyList());
        processList(Collections.singletonList("item"));
        
        // Collections.emptySet - 应该识别为 processSet(Set)
        processSet(Collections.emptySet());
        processSet(Collections.singleton("item"));
        
        // Collections.emptyMap - 应该识别为 processMap(Map)
        processMap(Collections.emptyMap());
        processMap(Collections.singletonMap("key", "value"));
    }
    
    public void testArraysAsList() {
        // Arrays.asList - 应该识别为 processList(List)
        processList(Arrays.asList("a", "b", "c"));
    }
    
    public void testNestedCalls() {
        // 嵌套调用 - 外层应该识别为 processList(List)
        processList(Lists.newArrayList(Arrays.asList("a", "b")));
    }
}
