package vn.sapo.omni.service.ddd;

public interface DomainRule {

    boolean isBroken();

    String message();
}
